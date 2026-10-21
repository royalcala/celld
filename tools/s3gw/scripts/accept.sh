#!/usr/bin/env bash
# Acceptance gate: start us-s3gw, run `celld diagnose` against it, and FAIL
# unless the output contains the literal line `ok bucket conditional write`.
#
# Usage: scripts/accept.sh            (or: just accept)
#   CELLD_BIN=/path/to/celld  override celld discovery
#   S3GW_PORT=9020            port to listen on
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

CELLD=$(t42_expect_celld)
echo "store:   $(t42_store_endpoint) (bucket s3://$T42_BUCKET)"
echo "celld:   $CELLD"

if t42_external_store; then
  echo "external store: S3GW_ENDPOINT is set, not starting a private us-s3gw"
else
  t42_start_store
  t42_ensure_bucket
  trap t42_stop_store EXIT
  echo "root:    $T42_STORE_DATA"
fi

set +e
OUT=$(
  AWS_ACCESS_KEY_ID="$T42_ACCESS" \
  AWS_SECRET_ACCESS_KEY="$T42_SECRET" \
  AWS_REGION="$T42_REGION" \
  AWS_DEFAULT_REGION="$T42_REGION" \
  AWS_EC2_METADATA_DISABLED=true \
  "$CELLD" diagnose \
    --bucket "s3://$T42_BUCKET" \
    --endpoint "$(t42_store_endpoint)" \
    --region "$T42_REGION" \
    --listen "${CELLD_DIAGNOSE_LISTEN:-127.0.0.1:0}" 2>&1
)
RC=$?
set -e

echo "----- celld diagnose (exit $RC) -----"
echo "$OUT"
echo "-------------------------------------"

if grep -q "ok bucket conditional write" <<<"$OUT"; then
  echo "PASS: celld diagnose reports 'ok bucket conditional write'"
  exit 0
fi

echo "FAIL: celld diagnose did NOT report 'ok bucket conditional write'" >&2
exit 1
