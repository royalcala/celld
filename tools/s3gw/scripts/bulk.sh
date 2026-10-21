#!/usr/bin/env bash
# Bulk-path measurement: single PUTs below celld's 5 MiB multipart threshold
# (L0 WAL capture ~512 KiB, deploy bundles) vs multipart uploads at/above it
# (what celld's LTX client does for >= 5 MiB, up to the 64 MiB compaction
# ceiling). Verifies every upload (sha256, ranged GET, multipart ETag).
#
# Usage: scripts/bulk.sh        (or: just bulk)
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

if t42_external_store; then
  echo "external store: S3GW_ENDPOINT is set, not starting a private us-s3gw"
else
  t42_start_store
  t42_ensure_bucket
  trap t42_stop_store EXIT
fi

python3 "$T42_SCRIPT_DIR/s3probe.py" bulk \
  --endpoint "$(t42_store_endpoint)" --bucket "$T42_BUCKET" \
  --access "$T42_ACCESS" --secret "$T42_SECRET" --region "$T42_REGION" \
  --single "${BULK_SINGLE:-512K,4M}" --multipart "${BULK_MULTIPART:-12M,64M}"
