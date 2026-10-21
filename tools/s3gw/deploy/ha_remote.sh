#!/usr/bin/env bash
# us-s3gw F1 on two real nodes — btrfs send/receive + failover/failback.
#
# Topology (T87):
#   server-1 (active)                               server-2 (standby)
#   /srv/us-s3gw  = btrfs subvol `data`             /srv/us-s3gw-recv/<snap>  (received)
#   /srv/us-s3gw-snaps = ro snapshots sent          (promote: snapshot -> `data`)
#
# Replication is FILESYSTEM-level: a read-only btrfs snapshot of the live
# subvolume, streamed with `btrfs send [-p parent]` over ssh into
# `btrfs receive`. The CAS is a process lock, so exactly ONE node may run the
# gateway: `promote` refuses while the local unit is active or while the peer
# answers `is-active` (two writers = two cell owners).
#
# Runs ON a node as root. Config comes from the instance env file
# (/etc/us-s3gw-ha/us-s3gw-ha.env): S3GW_{PORT,ROOT,BUCKET,ACCESS_KEY,SECRET_KEY,PEER_HOST}.
#
# Subcommands:
#   install      copy the unit into /run/systemd/system and daemon-reload
#   status       role facts: units, snapshots, received, usage
#   snapshot     take a read-only snapshot of the live root (no send)
#   replicate    snapshot + `btrfs send | ssh peer btrfs receive` (run on the ACTIVE node)
#   promote      fence peer -> make the newest received snapshot the live root,
#                start the gateway, run the gates (celld diagnose + race)  [--force]
#   gates        run the acceptance gates against the local gateway
#   demote       stop the local gateway (fence/standby)
set -euo pipefail

ENV_FILE="${S3GW_ENV_FILE:-/etc/us-s3gw-ha/us-s3gw-ha.env}"
[ -r "$ENV_FILE" ] || { echo "ha_remote: missing $ENV_FILE" >&2; exit 1; }
set -a; . "$ENV_FILE"; set +a

: "${S3GW_ROOT:=/srv/us-s3gw}"
: "${S3GW_PORT:=9120}"
: "${S3GW_BUCKET:=fleet}"
: "${S3GW_PEER_HOST:?S3GW_PEER_HOST (peer LAN IP) is required in the env file}"
SNAPS="${S3GW_SNAPS:-/srv/us-s3gw-snaps}"
RECV="${S3GW_RECV:-/srv/us-s3gw-recv}"
UNIT="${S3GW_UNIT:-us-s3gw-ha}"
STATE="${S3GW_STATE:-/var/lib/us-s3gw-ha}"
BTRFS="${S3GW_BTRFS:-/root/.nix-profile/bin/btrfs}"
PY="${S3GW_PY:-/root/.nix-profile/bin/python3}"
CELLD="${S3GW_CELLD:-$(ls -d /nix/store/*celld-0.6.0/bin/celld 2>/dev/null | head -1)}"
PROBE="${S3GW_PROBE:-/usr/local/share/us-s3gw-ha/s3probe.py}"
SSH_PEER=(ssh -o BatchMode=yes -o ConnectTimeout=10 "root@$S3GW_PEER_HOST")

log() { printf '%s\n' "$*" ; }
die() { printf 'ha_remote: %s\n' "$*" >&2; exit 1; }
now() { date -u +%Y%m%dT%H%M%SZ; }
millis() { date +%s%3N; }
secs() { awk "BEGIN{printf \"%.2f\", ($1-$2)/1000}" "$1" "$2"; }

require_peer() { "${SSH_PEER[@]}" true 2>/dev/null || die "peer $S3GW_PEER_HOST unreachable over ssh"; }

peer_unit_state() { "${SSH_PEER[@]}" "systemctl is-active $UNIT 2>/dev/null || true" | head -1; }

run_gates() {
  local t0 t1 rc=0
  [ -n "$CELLD" ] && [ -x "$CELLD" ] || die "celld binary not found (set S3GW_CELLD)"
  t0=$(millis)
  log "   gates: celld diagnose --bucket s3://$S3GW_BUCKET --endpoint http://127.0.0.1:$S3GW_PORT"
  local out
  out=$(AWS_ACCESS_KEY_ID="$S3GW_ACCESS_KEY" AWS_SECRET_ACCESS_KEY="$S3GW_SECRET_KEY" \
        AWS_REGION=us-east-1 AWS_DEFAULT_REGION=us-east-1 AWS_EC2_METADATA_DISABLED=true \
        "$CELLD" diagnose --bucket "s3://$S3GW_BUCKET" \
        --endpoint "http://127.0.0.1:$S3GW_PORT" --region us-east-1 \
        --listen 127.0.0.1:0 2>&1) || rc=1
  echo "$out" | sed 's/^/   /'
  if ! grep -q "ok bucket conditional write" <<<"$out"; then
    log "   FAIL: no 'ok bucket conditional write'"
    return 1
  fi
  log "   gates: scripts/race.sh = 24 x If-None-Match:* (s3probe)"
  "$PY" "$PROBE" race --endpoint "http://127.0.0.1:$S3GW_PORT" --bucket "$S3GW_BUCKET" \
        --access "$S3GW_ACCESS_KEY" --secret "$S3GW_SECRET_KEY" --n 24 --rounds 5 | sed 's/^/   /' || rc=1
  t1=$(millis)
  [ "$rc" = 0 ] || return 1
  log "   gates PASS in $(secs "$t1" "$t0")s"
}

newest_received() { ls -1 "$RECV" 2>/dev/null | grep -E '^2' | sort | tail -1; }

cmd_install() {
  [ -f /usr/local/share/us-s3gw-ha/us-s3gw-ha.service ] || die "unit file not found"
  cp /usr/local/share/us-s3gw-ha/us-s3gw-ha.service /run/systemd/system/"$UNIT".service
  systemctl daemon-reload
  log "install: /run/systemd/system/$UNIT.service (runtime; NixOS declarative equivalent in the runbook)"
}

cmd_status() {
  log "node             : $(hostname)  peer: $S3GW_PEER_HOST"
  log "unit             : $UNIT = $(systemctl is-active "$UNIT" 2>/dev/null || echo n/a)  (port $S3GW_PORT)"
  log "root             : $S3GW_ROOT  ($(findmnt -no SOURCE,FSTYPE,OPTIONS "$S3GW_ROOT" 2>/dev/null | head -1))"
  log "snapshots (sent) : $(ls -1 "$SNAPS" 2>/dev/null | wc -l)   last sent: $(cat "$STATE/last_sent" 2>/dev/null || echo -)"
  log "received         : $(ls -1 "$RECV" 2>/dev/null | wc -l)   newest: $(newest_received || echo -)"
  log "data usage       : $(du -sh "$S3GW_ROOT" 2>/dev/null | cut -f1)"
}

cmd_snapshot() {
  mkdir -p "$SNAPS"
  local snap="$SNAPS/$(now)"
  [ -e "$snap" ] && die "snapshot already exists: $snap"
  "$BTRFS" subvolume snapshot -r "$S3GW_ROOT" "$snap" >/dev/null
  log "snapshot: $snap ($(du -sh "$snap" | cut -f1))"
}

cmd_replicate() {
  require_peer
  [ "$(systemctl is-active "$UNIT" 2>/dev/null || true)" = active ] || log "replicate: NOTE local unit is not active ($(systemctl is-active "$UNIT" 2>/dev/null || true))"
  mkdir -p "$SNAPS" "$STATE"
  local t0 t1 snap name parent
  t0=$(millis)
  snap="$SNAPS/$(now)"
  "$BTRFS" subvolume snapshot -r "$S3GW_ROOT" "$snap" >/dev/null
  name="$(basename "$snap")"
  parent="$(cat "$STATE/last_sent" 2>/dev/null || true)"
  local send_args=()
  if [ -n "$parent" ] && [ -e "$SNAPS/$parent" ] && "${SSH_PEER[@]}" "test -e $RECV/$parent" 2>/dev/null; then
    send_args=(-p "$SNAPS/$parent")
    log "replicate: incremental from $parent"
  else
    log "replicate: full send"
  fi
  "${SSH_PEER[@]}" "mkdir -p $RECV"
  "$BTRFS" send "${send_args[@]}" "$snap" | "${SSH_PEER[@]}" "$BTRFS receive $RECV" >/dev/null
  "${SSH_PEER[@]}" "test -d $RECV/$name/$S3GW_BUCKET" || { "${SSH_PEER[@]}" "test -d $RECV/$name" || die "peer did not receive $name"; }
  printf '%s\n' "$name" >"$STATE/last_sent"
  t1=$(millis)
  log "replicate: ok  snapshot=$name files=$(find "$S3GW_ROOT" -type f | wc -l) bytes=$(du -sb "$S3GW_ROOT" | cut -f1) elapsed=$(secs "$t1" "$t0")s"
}

cmd_promote() {
  local force=0; [ "${1:-}" = --force ] && force=1
  [ "$(systemctl is-active "$UNIT" 2>/dev/null || true)" = active ] && die "local unit is already active: refusing a second writer"
  local peer_state; peer_state="$(peer_unit_state || true)"
  if [ "$peer_state" = active ]; then
    [ "$force" = 1 ] || die "PEER GATEWAY STILL ACTIVE on $S3GW_PEER_HOST: two writers would each hold their own CAS lock (two cell owners). Fence it first or pass --force"
    log "promote: WARNING --force with the peer active"
  fi
  local snap; snap="$(newest_received)"; [ -n "$snap" ] || die "no received snapshot in $RECV"
  [ -d "$RECV/$snap/$S3GW_BUCKET" ] || die "received snapshot $snap has no bucket dir"
  local t0 t1; t0=$(millis)
  log "promote: peer '$peer_state', promoting received snapshot $snap"
  # The data fs is on an LV on server-1 and on the partition on server-2:
  # resolve the btrfs UUID from the LIVE mount before unmounting anything.
  local uuid src
  src="$(findmnt -no SOURCE "$SNAPS" | head -1)"; src="${src%%[*}"   # strip the [/subvol] suffix
  uuid="$(blkid -s UUID -o value "$src" 2>/dev/null || true)"
  [ -n "$uuid" ] || die "cannot resolve the data fs UUID"
  systemctl stop "$UNIT" 2>/dev/null || true
  systemctl stop "$S3GW_ROOT" "$RECV" "$SNAPS" 2>/dev/null || true
  mkdir -p /mnt/us-s3gw-tmp
  mount -t btrfs "/dev/disk/by-uuid/$uuid" /mnt/us-s3gw-tmp
  if [ -e /mnt/us-s3gw-tmp/data ]; then mv /mnt/us-s3gw-tmp/data "/mnt/us-s3gw-tmp/data-old-$(now)"; fi
  "$BTRFS" subvolume snapshot "/mnt/us-s3gw-tmp/recv/$snap" /mnt/us-s3gw-tmp/data >/dev/null
  umount /mnt/us-s3gw-tmp; rmdir /mnt/us-s3gw-tmp
  systemctl start "$S3GW_ROOT" "$RECV" "$SNAPS"
  chown -h us-s3gw:us-s3gw "$S3GW_ROOT"
  systemctl start "$UNIT"
  for _ in $(seq 1 100); do ss -ltn | grep -q ":$S3GW_PORT " && break; sleep 0.1; done
  log "promote: gateway $UNIT up on :$S3GW_PORT (root $S3GW_ROOT)"
  mkdir -p "$STATE"; printf '%s\n' "$(now)" >"$STATE/promoted_at"
  if ! run_gates; then die "promotion FAILED its gates (gateway left running for inspection)"; fi
  t1=$(millis)
  log "promote: OK  total=$(secs "$t1" "$t0")s"
}

cmd_demote() {
  systemctl stop "$UNIT" 2>/dev/null || true
  log "demote: $UNIT stopped ($(systemctl is-active "$UNIT" 2>/dev/null || true))"
}

cmd_gates() { run_gates; }

main() {
  local cmd="${1:-}"; shift || true
  case "$cmd" in
    install) cmd_install "$@" ;;
    status) cmd_status "$@" ;;
    snapshot) cmd_snapshot "$@" ;;
    replicate) cmd_replicate "$@" ;;
    promote) cmd_promote "$@" ;;
    demote) cmd_demote "$@" ;;
    gates) cmd_gates "$@" ;;
    -h|--help|"") sed -n '2,40p' "$0" ;;
    *) die "unknown subcommand '$cmd' (install|status|snapshot|replicate|promote|demote|gates)" ;;
  esac
}
main "$@"
