#!/usr/bin/env bash
# us-s3gw restore — bring a root back from a backup.sh snapshot (or mirror).
#
#   restore.sh --snapshot DIR --target DIR [--force]
#
# Refuses a non-empty target unless --force (so a restore never merges into
# live data by accident). After it finishes:
#   1. start the service against --target,
#   2. `celld diagnose …` must print `ok bucket conditional write`,
#   3. `scripts/race.sh` must print one winner per round.
# The runbook has the full drill: 1us-infra/docs/fleet-storage-runbook.md.
set -euo pipefail

SNAP=""
TARGET=""
FORCE=0

while [ $# -gt 0 ]; do
  case "$1" in
    --snapshot) SNAP="$2"; shift 2 ;;
    --target) TARGET="$2"; shift 2 ;;
    --force) FORCE=1; shift ;;
    -h|--help) sed -n '2,14p' "$0"; exit 0 ;;
    *) echo "error: unknown argument: $1" >&2; exit 2 ;;
  esac
done

die() { echo "restore: $*" >&2; exit 1; }

[ -n "$SNAP" ] || die "--snapshot DIR is required"
[ -n "$TARGET" ] || die "--target DIR is required"
[ -d "$SNAP" ] || die "snapshot not found: $SNAP"
[ -n "$(ls -A "$SNAP")" ] || die "snapshot is empty: $SNAP"
command -v rsync >/dev/null || die "rsync is required"

if [ -d "$TARGET" ] && [ -n "$(ls -A "$TARGET")" ] && [ "$FORCE" != "1" ]; then
  die "target is not empty: $TARGET (pass --force to overwrite)"
fi

mkdir -p "$TARGET"
# MANIFEST describes the snapshot itself, not the store root.
rsync -a --delete --delete-excluded --exclude=/MANIFEST "$SNAP/" "$TARGET/"
echo "restore: ok  snapshot=$SNAP target=$TARGET files=$(find "$TARGET" -type f | wc -l) bytes=$(du -sb "$TARGET" | cut -f1)"
echo "restore: now start the service against $TARGET and run 'celld diagnose' + 'scripts/race.sh'"
