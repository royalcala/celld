#!/usr/bin/env bash
# us-s3gw backup — mirror the data dir and take a space-efficient point-in-time
# snapshot (rsync --link-dest, the poor-man's ZFS snapshot).
#
# F0 contract (spec 2026-10-02, D2/D4): us-s3gw is one process over one root;
# durability is delegated to the substrate, so a backup is a plain copy of that
# root. Run it from us-s3gw-backup.service / .timer, or by hand for a manual
# restore drill.
#
#   backup.sh [--root DIR] [--mirror DIR] [--snapshots DIR] [--retain N]
#             [--quiesce] [--scope user|system] [--service NAME]
#
# Reads the same S3GW_* environment as the service (us-s3gw.env):
#   S3GW_ROOT, S3GW_MIRROR, S3GW_SNAPSHOTS, S3GW_RETAIN (default 7),
#   S3GW_QUIESCE (0/1), S3GW_SYSTEMD_SCOPE (user|system), S3GW_SERVICE.
#
# S3GW_QUIESCE=1 stops the service for the copy (cold, always consistent);
# 0 is an online copy. Online is safe per file — the store writes every object
# via tmp+rename — but a snapshot taken mid-flight can pair an object's data
# with a newer metadata sidecar. Prefer quiesce for a backup you will restore.
set -euo pipefail

ROOT="${S3GW_ROOT:-}"
MIRROR="${S3GW_MIRROR:-}"
SNAPSHOTS="${S3GW_SNAPSHOTS:-}"
RETAIN="${S3GW_RETAIN:-7}"
QUIESCE="${S3GW_QUIESCE:-0}"
SCOPE="${S3GW_SYSTEMD_SCOPE:-system}"
SERVICE="${S3GW_SERVICE:-us-s3gw}"

while [ $# -gt 0 ]; do
  case "$1" in
    --root) ROOT="$2"; shift 2 ;;
    --mirror) MIRROR="$2"; shift 2 ;;
    --snapshots) SNAPSHOTS="$2"; shift 2 ;;
    --retain) RETAIN="$2"; shift 2 ;;
    --quiesce) QUIESCE=1; shift ;;
    --scope) SCOPE="$2"; shift 2 ;;
    --service) SERVICE="$2"; shift 2 ;;
    -h|--help) sed -n '2,25p' "$0"; exit 0 ;;
    *) echo "error: unknown argument: $1" >&2; exit 2 ;;
  esac
done

die() { echo "backup: $*" >&2; exit 1; }

[ -n "$ROOT" ] || die "S3GW_ROOT (or --root) is required"
[ -d "$ROOT" ] || die "root does not exist: $ROOT"
[ -n "$MIRROR$SNAPSHOTS" ] || die "set S3GW_MIRROR and/or S3GW_SNAPSHOTS (nothing to do)"
case "$RETAIN" in ''|*[!0-9]*) die "S3GW_RETAIN must be a number" ;; esac

command -v rsync >/dev/null || die "rsync is required"

control() {
  if [ "$SCOPE" = "user" ]; then systemctl --user "$@"; else systemctl "$@"; fi
}

# Serialize runs (timer + a manual drill must not overlap).
exec 9>"/tmp/us-s3gw-backup.lock"
flock -n 9 || die "another backup is already running"

STOPPED=0
restart_service() {
  if [ "$STOPPED" = "1" ]; then
    control start "$SERVICE" || echo "backup: WARNING could not restart $SERVICE" >&2
  fi
}
trap restart_service EXIT

if [ "$QUIESCE" = "1" ]; then
  echo "backup: stopping $SERVICE for a quiesced copy"
  control stop "$SERVICE"
  STOPPED=1
fi

EXCLUDES=(--exclude='.tmp.*.internal.part')

if [ -n "$MIRROR" ]; then
  mkdir -p "$MIRROR"
  rsync -a --delete "${EXCLUDES[@]}" "$ROOT/" "$MIRROR/"
  echo "backup: mirror ok  mirror=$MIRROR bytes=$(du -sb "$MIRROR" | cut -f1)"
fi

if [ -n "$SNAPSHOTS" ]; then
  mkdir -p "$SNAPSHOTS"
  ts="$(date -u +%Y%m%dT%H%M%SZ)"
  dest="$SNAPSHOTS/$ts"
  [ -e "$dest" ] && die "snapshot already exists: $dest"
  prev="$(find "$SNAPSHOTS" -mindepth 1 -maxdepth 1 -type d -name '2*' ! -name '*.inprogress' | sort | tail -1)"
  # Land in a temp dir, then rename: a snapshot appears complete or not at all.
  tmp="$dest.inprogress"
  mkdir -p "$tmp"
  if [ -n "$prev" ]; then
    rsync -a --delete --link-dest="$prev" "${EXCLUDES[@]}" "$ROOT/" "$tmp/"
  else
    rsync -a --delete "${EXCLUDES[@]}" "$ROOT/" "$tmp/"
  fi
  mv "$tmp" "$dest"
  {
    echo "timestamp=$ts"
    echo "host=$(hostname)"
    echo "root=$ROOT"
    echo "files=$(find "$dest" -type f | wc -l)"
    echo "bytes=$(du -sb "$dest" | cut -f1)"
  } >"$dest/MANIFEST"
  echo "backup: snapshot ok snapshot=$dest files=$(sed -n 's/^files=//p' "$dest/MANIFEST") bytes=$(sed -n 's/^bytes=//p' "$dest/MANIFEST")"

  # Retention: keep the newest $RETAIN, drop the rest (GNU head -n -N).
  find "$SNAPSHOTS" -mindepth 1 -maxdepth 1 -type d -name '2*' ! -name '*.inprogress' \
    | sort | head -n -"$RETAIN" | while IFS= read -r old; do
      rm -rf -- "$old"
    done
fi
