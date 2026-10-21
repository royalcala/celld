#!/usr/bin/env bash
# Mixed-version fleet: node A runs the pre-B build (token v1), node B the
# current build (token v2). Exercises the bounded rolling-upgrade window.
# Recommended local/CI endpoint: tools/s3gw (CAS-serialized conditional
# writes, hashed metadata filenames). PREB_BIN is a binary built at the
# pre-B commit; the script prints the observed compatibility errors.
set -euo pipefail
cd "$(dirname "$0")/.."

BIN=${CELLD_BIN:-target/debug/celld}
PREB=${PREB_BIN:-/tmp/celld-preb-target/debug/celld}
ENDPOINT=${S3_ENDPOINT:?set S3_ENDPOINT}
BUCKET=${S3_BUCKET:-celld-mixed}
LOCAL_ROOT=${S3_LOCAL_ROOT:-}
APORT=${A_PORT:-18302}; BPORT=${B_PORT:-18303}
AINT=${A_INTERNAL:-19302}; BINT=${B_INTERNAL:-19303}
TTL_MS=${CELLD_TTL_MS:-4000}
export AWS_ACCESS_KEY_ID=${AWS_ACCESS_KEY_ID:-minioadmin}
export AWS_SECRET_ACCESS_KEY=${AWS_SECRET_ACCESS_KEY:-minioadmin}
export AWS_REGION=${AWS_REGION:-us-east-1}
export CELLD_TTL_MS=$TTL_MS

log() { printf '%s\n' "== $*"; }
fail() { printf 'FAIL: %s\n' "$*" >&2; exit 1; }
[ -x "$PREB" ] || fail "missing pre-B binary at $PREB (PREB_BIN)"

owner_record() {
  [ -n "$LOCAL_ROOT" ] || return 1
  local f
  f=$(find "$LOCAL_ROOT/$BUCKET/cells" -path '*Cell:*' -name own.json 2>/dev/null | head -1)
  [ -n "$f" ] || return 1
  python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["node"])' "$f" 2>/dev/null
}
wait_for() {
  for _ in $(seq 1 120); do
    [ "$(curl -s --max-time 2 "$1" || true)" = "$2" ] && return 0
    sleep 0.25
  done
  return 1
}
wait_owner() {
  local deadline=$(( $(date +%s) + ${2:-30} ))
  while [ "$(date +%s)" -lt "$deadline" ]; do
    # Drive a request so the surviving node notices the expired owner lease.
    curl -s --max-time 5 "http://127.0.0.1:$BPORT/hello" >/dev/null 2>&1 || true
    [ "$(owner_record || true)" = "$1" ] && return 0
    sleep 0.25
  done
  return 1
}
start_a() { # binary
  CELLD_NODE=node-a "$1" --bucket "s3://$BUCKET" --endpoint "$ENDPOINT" \
    --listen "127.0.0.1:$APORT" --internal-listen "127.0.0.1:$AINT" --advertise "127.0.0.1:$AINT" \
    >/tmp/mixed-a.log 2>&1 & A_PID=$!
}
start_b() {
  CELLD_NODE=node-b "$BIN" --bucket "s3://$BUCKET" --endpoint "$ENDPOINT" \
    --listen "127.0.0.1:$BPORT" --internal-listen "127.0.0.1:$BINT" --advertise "127.0.0.1:$BINT" \
    >/tmp/mixed-b.log 2>&1 & B_PID=$!
}
cleanup() { kill -9 "${A_PID:-}" "${B_PID:-}" 2>/dev/null || true; }
trap cleanup EXIT

log "deploy fixture with the current build"
"$BIN" deploy examples/fleet-fixture/wrangler.jsonc --bucket "s3://$BUCKET" --endpoint "$ENDPOINT" >/tmp/mixed-deploy.log 2>&1 || { tail -5 /tmp/mixed-deploy.log; exit 1; }

log "start the pre-B node A first so it owns the cell"
start_a "$PREB"
wait_for "http://127.0.0.1:$APORT/hello" "hello-from-cell" || { tail -20 /tmp/mixed-a.log; fail "A not ready"; }
[ -n "$LOCAL_ROOT" ] || fail "set S3_LOCAL_ROOT for the ownership assertion"
wait_owner node-a 30 || fail "the pre-B node did not own the cell"

log "start B on the current build; the cell stays with A"
start_b
wait_for "http://127.0.0.1:$BPORT/hello" "hello-from-cell" || { tail -20 /tmp/mixed-b.log; fail "B not ready"; }
wait_owner node-a 30 || fail "the pre-B node lost the cell before the tests"

log "v1 durable handle stored on A, used on the new node B"
[ "$(curl -s "http://127.0.0.1:$APORT/store")" = "stored" ] || fail "store v1 durable"
DURABLE=$(curl -s "http://127.0.0.1:$BPORT/durable")
[ "$DURABLE" = "durable-value" ] || fail "v1 durable not replayed on new node: $DURABLE"

log "v1 transient handle from A rejected typed on B"
LEGACY=$(curl -s "http://127.0.0.1:$BPORT/legacy-cell")
[ "$LEGACY" = "err:forbidden" ] || fail "expected err:forbidden, got $LEGACY"

log "v2 transient handle from B rejected by the pre-B node A"
NEW_TO_OLD=$(curl -s "http://127.0.0.1:$BPORT/stateless")
[ "$NEW_TO_OLD" != "pong" ] || fail "pre-B node accepted a v2 transient handle"
log "new->old error: $NEW_TO_OLD"

log "upgrade A: kill it, let B take the cell, start A on the current build"
kill -9 "$A_PID"; wait "$A_PID" 2>/dev/null || true
wait_owner node-b 30 || fail "B did not take over"
start_a "$BIN"
wait_for "http://127.0.0.1:$APORT/hello" "hello-from-cell" || fail "A (upgraded) not ready"
wait_owner node-b 30 || fail "owner changed after the upgrade"
[ "$(curl -s "http://127.0.0.1:$BPORT/durable")" = "durable-value" ] || fail "durable lost after upgrade"
[ "$(curl -s "http://127.0.0.1:$BPORT/legacy-cell")" = "pong" ] || fail "homogeneous fleet cannot call a live callback"
[ "$(curl -s "http://127.0.0.1:$APORT/hello")" = "hello-from-cell" ] || fail "hello via upgraded A"

log "PASS mixed-version window"
