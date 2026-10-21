#!/usr/bin/env bash
# Concurrency gate: N (default 24) concurrent `If-None-Match: *` PUTs to the
# SAME key must produce exactly ONE winner per round (the process-wide CAS lock
# makes check+write atomic).
#
# Usage:
#   scripts/race.sh                     5 rounds, asserts 1 winner each (CI gate)
#   US_S3GW_NO_CAS_LOCK=1 scripts/race.sh --report-only
#       demo mode: same race with the lock OFF; reports winners, no assertion
#       (a >1 winner count here is the failure the lock prevents)
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

ARGS=(race --endpoint "$(t42_store_endpoint)" --bucket "$T42_BUCKET"
      --access "$T42_ACCESS" --secret "$T42_SECRET" --region "$T42_REGION"
      --n "${RACE_N:-24}" --rounds "${RACE_ROUNDS:-5}")
if [ "${1:-}" = "--report-only" ]; then
  ARGS+=(--report-only)
fi

if t42_external_store; then
  echo "external store: S3GW_ENDPOINT is set, not starting a private us-s3gw"
else
  t42_start_store
  t42_ensure_bucket
  trap t42_stop_store EXIT
fi

python3 "$T42_SCRIPT_DIR/s3probe.py" "${ARGS[@]}"
