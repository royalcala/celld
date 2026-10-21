#!/usr/bin/env bash
# us-s3gw F1 (HA) — replicate, failover, failback with ONE active writer.
#
# Topology this models (spec 2026-10-02 D4/D5):
#
#   nodo A (primary)                         nodo B (standby)
#   us-s3gw  ──▶  /var/lib/us-s3gw           replica of that root
#                     ^                        ^
#                     └── DRBD (block) or ZFS/btrfs send (fs) ──┘
#
# The CAS is a process-wide lock, so exactly ONE gateway may run at a time
# (`active_side` state, below). Replication is filesystem-level; promotion is
# "start the same unit over the replica". This script encodes that procedure,
# so the drill and the runbook cannot drift.
#
# Subcommands:
#   init                 create primary (btrfs subvolume if possible) + replica
#   status               role/epoch, unit state, replica freshness, drift
#   start | stop         start the gateway for the active side / stop both
#   replicate            fs snapshot of the primary + sync to the replica (active side only)
#   failover [--force]   fence primary -> promote replica -> start gateway -> gates
#   failback [--force]   stop promoted gateway -> sync back -> start primary -> gates
#
# Environment (all optional, with defaults):
#   S3GW_HA_DIR        $HOME/us-s3gw-f1          state + snapshots (NEVER inside a root)
#   S3GW_ROOT          <HA_DIR>/primary         live root of the active gateway
#   S3GW_STANDBY_ROOT  <HA_DIR>/replica         standby's copy of the root
#   S3GW_PRIMARY_PORT  9020 · S3GW_STANDBY_PORT 9021
#   S3GW_SERVICE       us-s3gw-primary          primary unit (production: us-s3gw)
#   S3GW_STANDBY_UNIT  us-s3gw-standby          unit started on promotion
#   S3GW_SYSTEMD_SCOPE user|system              default user
#   S3GW_BIN           ~/.local/bin/us-s3gw
#   S3GW_BUCKET        fleet
#   S3GW_ACCESS_KEY / S3GW_SECRET_KEY           the store key pair (required to start a gateway)
#   S3GW_MAX_STALENESS 900                      max replica age in seconds
#   S3GW_SNAPSHOT_MODE auto|btrfs|rsync         auto = btrfs when the root is a subvolume
#
# `celld diagnose` and `scripts/race.sh` are run as the promotion gate; they need
# CELLD_BIN (or the celld checkout) and the s3gw scripts, exactly as `just accept`.
set -euo pipefail

HA_DIR="${S3GW_HA_DIR:-$HOME/us-s3gw-f1}"
PRIMARY="${S3GW_ROOT:-$HA_DIR/primary}"
REPLICA="${S3GW_STANDBY_ROOT:-$HA_DIR/replica}"
SNAPS="$HA_DIR/snapshots"
STATE="$HA_DIR/state"
PRIMARY_PORT="${S3GW_PRIMARY_PORT:-9020}"
STANDBY_PORT="${S3GW_STANDBY_PORT:-9021}"
SCOPE="${S3GW_SYSTEMD_SCOPE:-user}"
PRIMARY_UNIT="${S3GW_SERVICE:-us-s3gw-primary}"
STANDBY_UNIT="${S3GW_STANDBY_UNIT:-us-s3gw-standby}"
BIN="${S3GW_BIN:-$HOME/.local/bin/us-s3gw}"
BUCKET="${S3GW_BUCKET:-fleet}"
MAX_STALENESS="${S3GW_MAX_STALENESS:-900}"
SNAP_MODE="${S3GW_SNAPSHOT_MODE:-auto}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

log() { printf '%s\n' "$*"; }
die() { printf 'ha: %s\n' "$*" >&2; exit 1; }
control() { if [ "$SCOPE" = user ]; then systemctl --user "$@"; else systemctl "$@"; fi; }

# ── state ────────────────────────────────────────────────────────────────────
active_side() { [ -f "$STATE/active_side" ] && cat "$STATE/active_side" || echo primary; }
epoch() { [ -f "$STATE/epoch" ] && cat "$STATE/epoch" || echo 1; }
set_active() { printf '%s\n' "$1" >"$STATE/active_side"; }
bump_epoch() { local e; e=$(( $(epoch) + 1 )); printf '%s\n' "$e" >"$STATE/epoch"; echo "$e"; }

# Tree hash: names + contents, order- and mtime-normalized (tar is enough here).
hash_tree() { tar -C "$1" --sort=name --mtime=@0 --owner=0 --group=0 --numeric-owner -cf - . | sha256sum | cut -d' ' -f1; }
count_files() { find "$1" -type f 2>/dev/null | wc -l; }
dir_bytes() { du -sb "$1" 2>/dev/null | cut -f1; }

gateway_active() { [ "$(control is-active "$1" 2>/dev/null)" = active ]; }
wait_port() {
  local port="$1" i
  for i in $(seq 1 100); do
    (exec 3<>"/dev/tcp/127.0.0.1/$port") 2>/dev/null && { exec 3>&- 3<&-; return 0; }
    sleep 0.1
  done
  return 1
}

require_key() {
  [ -n "${S3GW_ACCESS_KEY:-}" ] && [ -n "${S3GW_SECRET_KEY:-}" ] \
    || die "S3GW_ACCESS_KEY/S3GW_SECRET_KEY are required to start a gateway"
  [ -x "$BIN" ] || die "gateway binary not executable: $BIN (install deploy/, see deploy/README.md)"
}

start_gateway() { # unit root port
  local unit="$1" root="$2" port="$3"
  control stop "$unit" >/dev/null 2>&1 || true
  local extra=()
  [ "$SCOPE" = user ] && extra+=(--user)
  systemd-run "${extra[@]}" --unit "$unit" --collect \
    -p Restart=on-failure -p TimeoutStopSec=15 -p NoNewPrivileges=yes \
    "$BIN" --host 127.0.0.1 --port "$port" \
    --access-key "$S3GW_ACCESS_KEY" --secret-key "$S3GW_SECRET_KEY" "$root" >/dev/null
  wait_port "$port" || { control status "$unit" --no-pager >&2 || true; die "gateway $unit did not listen on :$port"; }
}
stop_gateway() {
  local unit="$1"
  gateway_active "$unit" || return 0
  control stop "$unit" || die "could not stop $unit"
  local i; for i in $(seq 1 50); do gateway_active "$unit" || return 0; sleep 0.1; done
  die "gateway $unit is still active"
}

# The promotion gate: celld's own acceptance test + the CAS race.
run_gates() { # endpoint
  local endpoint="$1"
  log "   gates against $endpoint"
  S3GW_ENDPOINT="$endpoint" S3GW_BUCKET="$BUCKET" "$REPO_ROOT/scripts/accept.sh" | sed 's/^/   /'
  S3GW_ENDPOINT="$endpoint" S3GW_BUCKET="$BUCKET" "$REPO_ROOT/scripts/race.sh" | grep -E '^(PASS|FAIL|round)' | sed 's/^/   /'
}

snapshot_primary() { # echo the snapshot path (or the live root when not btrfs)
  local ts="$1"
  # `btrfs subvolume show` needs root (b-tree search); snapshotting does not, so
  # try the snapshot itself and fall back to the live root when it is refused.
  if [ "$SNAP_MODE" != rsync ] && command -v btrfs >/dev/null 2>&1 \
     && btrfs subvolume snapshot -r "$PRIMARY" "$SNAPS/$ts" >/dev/null 2>&1; then
    printf '%s' "$SNAPS/$ts"
  else
    printf '%s' "$PRIMARY"
  fi
}

# ── subcommands ─────────────────────────────────────────────────────────────
cmd_init() {
  mkdir -p "$HA_DIR" "$SNAPS" "$STATE" "$REPLICA/$BUCKET"
  if [ ! -e "$PRIMARY" ]; then
    if [ "$SNAP_MODE" != rsync ] && command -v btrfs >/dev/null 2>&1; then
      btrfs subvolume create "$PRIMARY" >/dev/null 2>&1 || mkdir -p "$PRIMARY"
    else
      mkdir -p "$PRIMARY"
    fi
  fi
  mkdir -p "$PRIMARY/$BUCKET"
  [ -f "$STATE/active_side" ] || set_active primary
  [ -f "$STATE/epoch" ] || printf '1\n' >"$STATE/epoch"
  log "init: primary=$PRIMARY ($(command -v btrfs >/dev/null 2>&1 && echo 'btrfs available: FS-level snapshots' || echo 'no btrfs: replicate copies the live root'))"
  log "      replica=$REPLICA  state=$STATE  active_side=$(active_side) epoch=$(epoch)"
}

cmd_start() {
  require_key
  if [ "$(active_side)" = primary ]; then
    start_gateway "$PRIMARY_UNIT" "$PRIMARY" "$PRIMARY_PORT"
    log "start: '$PRIMARY_UNIT' serving $PRIMARY on :$PRIMARY_PORT (active_side=primary)"
  else
    start_gateway "$STANDBY_UNIT" "$REPLICA" "$STANDBY_PORT"
    log "start: '$STANDBY_UNIT' serving $REPLICA on :$STANDBY_PORT (active_side=replica)"
  fi
}

cmd_stop() {
  stop_gateway "$PRIMARY_UNIT"
  stop_gateway "$STANDBY_UNIT"
  log "stop: gateways stopped (none of this HA group is running)"
}

cmd_status() {
  log "active_side : $(active_side)   epoch: $(epoch)"
  log "primary unit: $PRIMARY_UNIT = $(control is-active "$PRIMARY_UNIT" 2>/dev/null || echo n/a) (:${PRIMARY_PORT})"
  log "standby unit: $STANDBY_UNIT = $(control is-active "$STANDBY_UNIT" 2>/dev/null || echo n/a) (:${STANDBY_PORT})"
  if [ -f "$STATE/REPLICA_STATE" ]; then
    log "replica state:"; sed 's/^/  /' "$STATE/REPLICA_STATE"
    local ts age now
    ts=$(sed -n 's/^timestamp_epoch=//p' "$STATE/REPLICA_STATE")
    now=$(date +%s); age=$(( now - ${ts:-0} ))
    log "replica age : ${age}s (max ${MAX_STALENESS}s)"
  else
    log "replica state: (none — run 'ha.sh replicate')"
  fi
  printf 'primary : files=%s bytes=%s\n' "$(count_files "$PRIMARY")" "$(dir_bytes "$PRIMARY")"
  printf 'replica : files=%s bytes=%s\n' "$(count_files "$REPLICA")" "$(dir_bytes "$REPLICA")"
}

cmd_replicate() {
  [ "$(active_side)" = primary ] || die "active side is '$(active_side)'; refusing to overwrite it with primary data (split-brain guard)"
  command -v rsync >/dev/null || die "rsync is required"
  mkdir -p "$SNAPS" "$STATE" "$REPLICA"
  local ts snap e
  ts="$(date -u +%Y%m%dT%H%M%SZ)"
  e="$(epoch)"
  snap="$(snapshot_primary "$ts")"
  [ "$snap" = "$PRIMARY" ] && log "replicate: no btrfs subvolume at $PRIMARY — syncing the live root" \
                           || log "replicate: fs-level snapshot $snap"
  rsync -a --delete --exclude='.tmp.*.internal.part' "$snap/" "$REPLICA/"
  diff -r --no-dereference "$snap" "$REPLICA" >/dev/null || die "replica differs from the snapshot after rsync"
  local hash; hash="$(hash_tree "$REPLICA")"
  {
    echo "epoch=$e"
    echo "timestamp_utc=$ts"
    echo "timestamp_epoch=$(date +%s)"
    echo "source=$snap"
    echo "files=$(count_files "$REPLICA")"
    echo "bytes=$(dir_bytes "$REPLICA")"
    echo "hash=$hash"
  } >"$STATE/REPLICA_STATE"
  log "replicate: ok  epoch=$e files=$(count_files "$REPLICA") bytes=$(dir_bytes "$REPLICA") hash=$hash"
}

cmd_failover() {
  local force=0; [ "${1:-}" = --force ] && force=1
  [ "$(active_side)" = primary ] || die "active side is already '$(active_side)'; refusing a second promotion (split-brain guard)"
  [ -f "$STATE/REPLICA_STATE" ] || die "no replica state; run 'ha.sh replicate' on the primary first"

  # Fence: the CAS is a process lock, so the old writer must be gone.
  if gateway_active "$PRIMARY_UNIT"; then
    if [ "$force" = 1 ]; then
      log "failover: WARNING --force with $PRIMARY_UNIT still active; stopping it now"
      stop_gateway "$PRIMARY_UNIT"
    else
      die "primary gateway '$PRIMARY_UNIT' is STILL ACTIVE: two writers would each hold their own CAS lock (two cell owners). Fence it first (systemctl $SCOPE stop $PRIMARY_UNIT) or pass --force only if the host is truly unreachable"
    fi
  fi

  local ts age now
  ts=$(sed -n 's/^timestamp_epoch=//p' "$STATE/REPLICA_STATE")
  now=$(date +%s); age=$(( now - ${ts:-0} ))
  if [ "$age" -gt "$MAX_STALENESS" ] && [ "$force" = 0 ]; then
    die "replica is stale (${age}s > ${MAX_STALENESS}s); replicate again or pass --force (data loss window)"
  fi

  [ -d "$REPLICA/$BUCKET" ] || die "replica has no bucket dir $REPLICA/$BUCKET"
  require_key
  local e; e="$(bump_epoch)"
  log "failover: promoting replica -> epoch=$e (was ${age}s behind)"
  start_gateway "$STANDBY_UNIT" "$REPLICA" "$STANDBY_PORT"
  log "failover: gateway '$STANDBY_UNIT' is serving $REPLICA on :$STANDBY_PORT"
  if ! run_gates "http://127.0.0.1:$STANDBY_PORT"; then
    stop_gateway "$STANDBY_UNIT"
    die "promotion FAILED its gates; gateway stopped, active_side stays primary"
  fi
  set_active replica
  { echo "epoch=$e"; echo "timestamp_utc=$(date -u +%Y%m%dT%H%M%SZ)"; echo "from=$PRIMARY"; echo "to=$REPLICA"; echo "unit=$STANDBY_UNIT"; echo "port=$STANDBY_PORT"; } >"$STATE/FAILOVER_STATE"
  log "failover: OK — active side is now the replica (epoch $e)"
}

cmd_failback() {
  local force=0; [ "${1:-}" = --force ] && force=1
  [ "$(active_side)" = replica ] || die "active side is '$(active_side)', not the replica; nothing to fail back"
  stop_gateway "$STANDBY_UNIT"
  log "failback: promoted gateway stopped; syncing replica -> primary (production: DRBD bitmap resync / zfs incremental)"
  rsync -a --delete --exclude='.tmp.*.internal.part' "$REPLICA/" "$PRIMARY/"
  diff -r --no-dereference "$REPLICA" "$PRIMARY" >/dev/null || die "primary differs from the replica after rsync"
  require_key
  local e; e="$(bump_epoch)"
  set_active primary
  log "failback: starting primary gateway '$PRIMARY_UNIT' on :$PRIMARY_PORT (epoch $e)"
  start_gateway "$PRIMARY_UNIT" "$PRIMARY" "$PRIMARY_PORT"
  if ! run_gates "http://127.0.0.1:$PRIMARY_PORT"; then
    die "failback gates FAILED; gateway left on :$PRIMARY_PORT for inspection"
  fi
  { echo "epoch=$e"; echo "timestamp_utc=$(date -u +%Y%m%dT%H%M%SZ)"; echo "from=$REPLICA"; echo "to=$PRIMARY"; echo "unit=$PRIMARY_UNIT"; echo "port=$PRIMARY_PORT"; } >"$STATE/FAILBACK_STATE"
  log "failback: OK — active side is the primary again (epoch $e)"
}

main() {
  local cmd="${1:-}"; shift || true
  case "$cmd" in
    init) cmd_init "$@" ;;
    status) cmd_status "$@" ;;
    start) cmd_start "$@" ;;
    stop) cmd_stop "$@" ;;
    replicate) cmd_replicate "$@" ;;
    failover) cmd_failover "$@" ;;
    failback) cmd_failback "$@" ;;
    -h|--help|"") sed -n '2,47p' "$0" ;;
    *) die "unknown subcommand '$cmd' (init|status|start|stop|replicate|failover|failback)" ;;
  esac
}

# Serialize HA operations (a drill and a timer must never overlap).
mkdir -p "$HA_DIR" 2>/dev/null || true
exec 9>"$HA_DIR/.lock"
flock -n 9 || die "another HA operation is running"

main "$@"
