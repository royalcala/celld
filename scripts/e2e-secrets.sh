#!/usr/bin/env bash
# Fleet secrets E2E: deploy `secret_text` + `secret_key`, prove the bucket and
# the manifest hold no plaintext, prove the right node serves them, prove a
# wrong/missing CELLD_SECRET_KEY refuses the deployment with a clear error, and
# rotate a value to a new deployment version.
#
# Requires an S3-compatible endpoint with conditional writes (tools/s3gw is the
# local/CI one) and esbuild. S3_LOCAL_ROOT, when set, is the gateway's data
# directory so the test can also inspect the bytes it stored on disk.
set -euo pipefail

BIN=${CELLD_BIN:-target/debug/celld}
ENDPOINT=${S3_ENDPOINT:?set S3_ENDPOINT}
BUCKET=${S3_BUCKET:?set S3_BUCKET}
ESBUILD=${CELLD_ESBUILD:-}
ROOT=${S3_LOCAL_ROOT:-}
PREB_BIN=${PREB_BIN:-}
DIR=$(mktemp -d)
TOKEN_FILE=$DIR/token
KEY_FILE=$DIR/key
DEPLOY_JSON=$DIR/deploy.json
PIDS=()
# The local/CI gateway (tools/s3gw) uses these; export so the deploy and every
# node process sign with the same credentials.
export AWS_ACCESS_KEY_ID=${AWS_ACCESS_KEY_ID:-minioadmin}
export AWS_SECRET_ACCESS_KEY=${AWS_SECRET_ACCESS_KEY:-minioadmin}
export AWS_REGION=${AWS_REGION:-us-east-1}
cleanup() {
  for pid in "${PIDS[@]:-}"; do
    kill -9 "$pid" 2>/dev/null || true
  done
  rm -rf "$DIR"
}
trap cleanup EXIT

TOKEN="sk-live-$(python3 -c 'import secrets; print(secrets.token_hex(24))')"
TOKEN2="sk-live-$(python3 -c 'import secrets; print(secrets.token_hex(24))')"
printf '%s' "$TOKEN" > "$TOKEN_FILE"
python3 -c 'import sys; open(sys.argv[1], "wb").write(bytes(range(32)))' "$KEY_FILE"
KEY=$(python3 -c 'import secrets; print(secrets.token_hex(32))')
WRONG_KEY=$(python3 -c 'import secrets; print(secrets.token_hex(32))')

deploy() { # token-file out-json
  CELLD_SECRET_KEY=$KEY CELLD_ESBUILD=$ESBUILD "$BIN" deploy \
    examples/secrets-fixture/wrangler.jsonc \
    --bucket "s3://$BUCKET" --endpoint "$ENDPOINT" \
    --secret "API_TOKEN=@$1" --secret "TLS_KEY=@$KEY_FILE" --json > "$2"
}

port=${SECRETS_PORT:-18402}
internal=$((port + 1000))
wrong_port=$((port + 1))
missing_port=$((port + 2))
preb_port=$((port + 3))

start_node() { # name port internal env...
  local name=$1 port=$2 internal=$3
  shift 3
  env CELLD_NODE="$name" "$@" "$BIN" \
    --bucket "s3://$BUCKET" --endpoint "$ENDPOINT" \
    --listen "127.0.0.1:$port" --internal-listen "127.0.0.1:$internal" \
    --advertise "127.0.0.1:$internal" >"$DIR/$name.log" 2>&1 &
  PIDS+=("$!")
  echo "$!"
}

wait_http() { # url
  for _ in $(seq 1 150); do
    if curl -fsS --max-time 1 "$1" >/dev/null 2>&1; then return 0; fi
    sleep 0.2
  done
  return 1
}

wait_log() { # file pattern
  for _ in $(seq 1 100); do
    if grep -Eq "$2" "$1" 2>/dev/null; then return 0; fi
    sleep 0.2
  done
  return 1
}

assert_no_plaintext() { # path
  if grep -rqF --binary-files=without-match "$TOKEN" "$1" 2>/dev/null; then
    echo "FAIL: plaintext token found under $1" >&2
    exit 1
  fi
  if grep -rqF "$(base64 -w0 "$KEY_FILE" 2>/dev/null || true)" "$1" 2>/dev/null; then
    echo "FAIL: plaintext key found under $1" >&2
    exit 1
  fi
}

echo "== deploy with secrets"
deploy "$TOKEN_FILE" "$DEPLOY_JSON"
VERSION=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' "$DEPLOY_JSON")
if grep -qF "$TOKEN" "$DEPLOY_JSON"; then
  echo "FAIL: deploy --json leaked a secret" >&2
  exit 1
fi
echo "   version=$VERSION (deploy output has no plaintext)"

if [ -n "$ROOT" ]; then
  echo "== bucket stores ciphertext only"
  assert_no_plaintext "$ROOT/$BUCKET"
  MANIFEST=$(find "$ROOT/$BUCKET" -name manifest.json -type f | head -1)
  [ -n "$MANIFEST" ] || { echo "FAIL: no manifest in bucket" >&2; exit 1; }
  grep -q '"secrets"' "$MANIFEST" || { echo "FAIL: manifest has no secrets ref" >&2; exit 1; }
  find "$ROOT/$BUCKET" -name secrets.json -type f | head -1 | grep -q . \
    || { echo "FAIL: no encrypted secrets blob" >&2; exit 1; }
fi

echo "== correct key serves both bindings"
start_node secrets-ok "$port" "$internal" CELLD_SECRET_KEY="$KEY" >/dev/null
wait_http "http://127.0.0.1:$port/token" || { cat "$DIR/secrets-ok.log" >&2; exit 1; }
got=$(curl -fsS "http://127.0.0.1:$port/token")
[ "$got" = "$TOKEN" ] || { echo "FAIL: token mismatch" >&2; exit 1; }
got=$(curl -fsS "http://127.0.0.1:$port/key")
[ "$got" = "32:0,1,2,3" ] || { echo "FAIL: key binding mismatch: $got" >&2; exit 1; }
if grep -qF "$TOKEN" "$DIR/secrets-ok.log"; then
  echo "FAIL: node log contains the secret" >&2
  exit 1
fi
echo "   token and 32-byte secret_key served; no secret in node log"

echo "== request id, spans, RPC/stream metrics"
RID="e2e-$(python3 -c 'import secrets; print(secrets.token_hex(8))')"
curl -fsS -D "$DIR/trace.headers" -H "x-celld-request-id: $RID" \
  "http://127.0.0.1:$port/trace" > "$DIR/trace.json"
grep -qi "^x-celld-request-id: $RID" "$DIR/trace.headers" \
  || { echo "FAIL: response is missing the request id" >&2; exit 1; }
python3 - "$DIR/trace.json" "$RID" <<'PY' || exit 1
import json, sys
doc = json.load(open(sys.argv[1]))
assert doc.get("traced") is True, doc
assert doc.get("name") == "e2e-span", doc
assert doc.get("requestId") == sys.argv[2], doc
PY
[ "$(curl -fsS "http://127.0.0.1:$port/rpc")" = "pong" ] \
  || { echo "FAIL: cross-isolate RPC failed" >&2; exit 1; }
[ "$(curl -fsS "http://127.0.0.1:$port/stream")" = "stream-data" ] \
  || { echo "FAIL: cross-isolate stream failed" >&2; exit 1; }
wait_log "$DIR/secrets-ok.log" 'span_begin' \
  || { cat "$DIR/secrets-ok.log" >&2; echo "FAIL: no span_begin log" >&2; exit 1; }
grep -q "$RID" "$DIR/secrets-ok.log" \
  || { echo "FAIL: span log has no request id" >&2; exit 1; }
grep -q '\[redacted\]' "$DIR/secrets-ok.log" \
  || { echo "FAIL: span attribute secret was not redacted" >&2; exit 1; }
curl -fsS "http://127.0.0.1:$internal/metrics" > "$DIR/metrics" \
  || { echo "FAIL: /metrics unavailable" >&2; exit 1; }
for metric in celld_http_requests_total celld_rpc_calls_total \
  celld_stream_reads_total celld_trace_spans_total; do
  grep -q "^$metric" "$DIR/metrics" \
    || { echo "FAIL: metric $metric missing" >&2; cat "$DIR/metrics" >&2; exit 1; }
done
echo "   request id echoed; span logged (secret attr redacted); metrics present"

echo "== wrong key refuses the deployment"
start_node secrets-wrong "$wrong_port" "$((wrong_port + 1000))" CELLD_SECRET_KEY="$WRONG_KEY" >/dev/null
wait_log "$DIR/secrets-wrong.log" 'encrypted secrets|authentication failed|CELLD_SECRET_KEY' \
  || { cat "$DIR/secrets-wrong.log" >&2; echo "FAIL: wrong key did not fail clearly" >&2; exit 1; }
if curl -fsS --max-time 1 "http://127.0.0.1:$wrong_port/token" 2>/dev/null | grep -qF "$TOKEN"; then
  echo "FAIL: wrong-key node served a secret" >&2
  exit 1
fi
if grep -qF "$TOKEN" "$DIR/secrets-wrong.log"; then
  echo "FAIL: wrong-key node log contains the secret" >&2
  exit 1
fi
echo "   wrong key rejected with a redacted error"

echo "== missing key refuses the deployment"
start_node secrets-missing "$missing_port" "$((missing_port + 1000))" >/dev/null
wait_log "$DIR/secrets-missing.log" 'CELLD_SECRET_KEY' \
  || { cat "$DIR/secrets-missing.log" >&2; echo "FAIL: missing key did not fail clearly" >&2; exit 1; }
echo "   missing key rejected with a clear error"

if [ -n "$PREB_BIN" ] && [ -x "$PREB_BIN" ]; then
  echo "== pre-C node behavior (informational)"
  start_node secrets-preb "$preb_port" "$((preb_port + 1000))" CELLD_SECRET_KEY="$KEY" >/dev/null
  # Deployment adoption can be lazy; a request forces the load and the
  # required-features check.
  for _ in $(seq 1 50); do
    curl -s --max-time 1 "http://127.0.0.1:$preb_port/" >/dev/null 2>&1 || true
    grep -Eq 'secrets-v1|unsupported.*feature' "$DIR/secrets-preb.log" 2>/dev/null && break
    sleep 0.2
  done
  if grep -Eq 'secrets-v1|unsupported.*feature' "$DIR/secrets-preb.log" 2>/dev/null; then
    echo "   pre-C node refused secrets-v1"
  else
    echo "   NOTE: this pre-C build predates the required-features check (or the"
    echo "         probe did not reach adoption); it can serve the deployment with"
    echo "         the secret bindings missing. Upgrade every node to C before the"
    echo "         first secrets deploy (docs/SECRETS.md)."
  fi
fi

echo "== rotation: a new value is a new version"
printf '%s' "$TOKEN2" > "$TOKEN_FILE"
deploy "$TOKEN_FILE" "$DEPLOY_JSON"
VERSION2=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' "$DEPLOY_JSON")
[ "$VERSION" != "$VERSION2" ] || { echo "FAIL: rotation reused the version" >&2; exit 1; }
# The node adopts a new deployment at its next pointer poll (default 30s), so
# wait longer than one poll before calling rotation broken.
for _ in $(seq 1 240); do
  got=$(curl -fsS --max-time 1 "http://127.0.0.1:$port/token" 2>/dev/null || true)
  [ "$got" = "$TOKEN2" ] && break
  sleep 0.25
done
[ "$got" = "$TOKEN2" ] || { echo "FAIL: node did not adopt the rotated secret" >&2; exit 1; }
echo "   version $VERSION -> $VERSION2; rotated secret served"

echo "PASS secrets E2E"
