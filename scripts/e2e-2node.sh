#!/usr/bin/env bash
# Two-node runtime E2E. Requires an S3-compatible endpoint (MinIO or s3s-fs).
set -euo pipefail
cd "$(dirname "$0")/.."

BIN=${CELLD_BIN:-target/debug/celld}
ENDPOINT=${S3_ENDPOINT:?set S3_ENDPOINT}
BUCKET=${S3_BUCKET:-celld-fleet}
APORT=${A_PORT:-18202}; BPORT=${B_PORT:-18203}
AINT=${A_INTERNAL:-19202}; BINT=${B_INTERNAL:-19203}
TTL_MS=${CELLD_TTL_MS:-4000}
export AWS_ACCESS_KEY_ID=${AWS_ACCESS_KEY_ID:-minioadmin}
export AWS_SECRET_ACCESS_KEY=${AWS_SECRET_ACCESS_KEY:-minioadmin}
export AWS_REGION=${AWS_REGION:-us-east-1}
export CELLD_TTL_MS=$TTL_MS

log() { printf '%s\n' "== $*"; }
fail() { printf 'FAIL: %s\n' "$*" >&2; exit 1; }
wait_for() { # url expected
  for _ in $(seq 1 120); do
    body=$(curl -s --max-time 2 "$1" || true)
    if [ "$body" = "$2" ]; then return 0; fi
    sleep 0.25
  done
  return 1
}

log "deploy fixture"
"$BIN" deploy examples/fleet-fixture/wrangler.jsonc --bucket "s3://$BUCKET" --endpoint "$ENDPOINT" >/tmp/fleet-deploy.log 2>&1 || { tail -5 /tmp/fleet-deploy.log; exit 1; }

start_a() {
  CELLD_NODE=node-a "$BIN" --bucket "s3://$BUCKET" --endpoint "$ENDPOINT" \
    --listen "127.0.0.1:$APORT" --internal-listen "127.0.0.1:$AINT" --advertise "127.0.0.1:$AINT" \
    >/tmp/fleet-a.log 2>&1 &
  A_PID=$!
}
start_b() {
  CELLD_NODE=node-b "$BIN" --bucket "s3://$BUCKET" --endpoint "$ENDPOINT" \
    --listen "127.0.0.1:$BPORT" --internal-listen "127.0.0.1:$BINT" --advertise "127.0.0.1:$BINT" \
    >/tmp/fleet-b.log 2>&1 &
  B_PID=$!
}
cleanup() { kill -9 "${A_PID:-}" "${B_PID:-}" 2>/dev/null || true; }
trap cleanup EXIT

start_a; start_b
log "wait for readiness"
wait_for "http://127.0.0.1:$APORT/hello" "hello-from-cell" || { tail -20 /tmp/fleet-a.log; fail "node A not ready"; }
wait_for "http://127.0.0.1:$BPORT/hello" "hello-from-cell" || { tail -20 /tmp/fleet-b.log; fail "node B not ready"; }

log "cross-node capability"
[ "$(curl -s "http://127.0.0.1:$BPORT/hello")" = "hello-from-cell" ] || fail "cross-node hello"

log "cross-node stream (BYOB)"
[ "$(curl -s "http://127.0.0.1:$BPORT/stream")" = "cross-node-stream" ] || fail "cross-node stream"

log "cross-node durable restore"
[ "$(curl -s "http://127.0.0.1:$APORT/store")" = "stored" ] || fail "store durable"
[ "$(curl -s "http://127.0.0.1:$BPORT/durable")" = "durable-value" ] || fail "durable before failover"

log "stateless capability across nodes is rejected typed"
STATELESS=$(curl -s "http://127.0.0.1:$BPORT/stateless")
[ "$STATELESS" = "err:owner-moved" ] || fail "expected err:owner-moved, got $STATELESS"

log "cross-node cancellation"
curl -s --max-time 0.25 "http://127.0.0.1:$BPORT/slow" >/dev/null 2>&1 || true
sleep 2
[ "$(curl -s "http://127.0.0.1:$BPORT/flag?key=slow")" = "false" ] || fail "cancellation did not cross the forward"

log "kill node A, wait for takeover"
kill -9 "$A_PID"; wait "$A_PID" 2>/dev/null || true
sleep $((TTL_MS / 1000 + 6))
[ "$(curl -s --max-time 20 "http://127.0.0.1:$BPORT/durable")" = "durable-value" ] || fail "durable takeover"
[ "$(curl -s "http://127.0.0.1:$BPORT/hello")" = "hello-from-cell" ] || fail "hello after takeover"

log "restart node A: the cell stays with one owner"
start_a
wait_for "http://127.0.0.1:$APORT/hello" "hello-from-cell" || fail "node A not ready after restart"
[ "$(curl -s "http://127.0.0.1:$APORT/hello")" = "hello-from-cell" ] || fail "hello via restarted A"

log "PASS 2-node matrix"
