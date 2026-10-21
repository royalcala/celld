#!/usr/bin/env bash
# Live measurement: run a REAL celld node against us-s3gw and report the
# object sizes the fleet bucket actually stores (T42 task 3).
#
# What it does:
#   1. starts us-s3gw (debug log) with a throwaway root,
#   2. `celld deploy` of scripts/live-app (a DO that absorbs chunks),
#   3. starts a production celld node on 127.0.0.1:$LIVE_NODE_PORT,
#   4. small-cell phase + growth phase: fills an ~8 MiB cell and keeps it
#      dirty until the node's L1 compaction fires (default triggers: 256
#      txids or 32 MiB pending),
#   5. census of every object + PASS/FAIL: an object >= 5 MiB must exist and
#      us-s3gw must have served it through multipart (celld's client switches
#      to 5 MiB parts at MULTIPART_THRESHOLD, crates/ltx/src/client/object_store.rs).
#
# Requires: built us-s3gw, a celld binary (CELLD_BIN or celld/target/*), and
# esbuild on PATH. Leaves its work dir behind (prints the path); the store and
# node are always stopped.
#
# Usage: scripts/live-sizes.sh        (or: just live)
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

CELLD=$(t42_expect_celld)
command -v esbuild >/dev/null 2>&1 || t42_die "esbuild not on PATH (required by 'celld deploy')"
command -v python3 >/dev/null 2>&1 || t42_die "python3 not on PATH"
BIN=$(t42_ensure_store_bin)

NODE_PORT="${LIVE_NODE_PORT:-9897}"
APP="$T42_SCRIPT_DIR/live-app"
WORK=$(mktemp -d "${TMPDIR:-/tmp}/us-s3gw-live.XXXXXX")
mkdir -p "$WORK/root/$T42_BUCKET"
export AWS_ACCESS_KEY_ID="$T42_ACCESS" AWS_SECRET_ACCESS_KEY="$T42_SECRET"
export AWS_REGION="$T42_REGION" AWS_DEFAULT_REGION="$T42_REGION"
export AWS_EC2_METADATA_DISABLED=true

STORE_PID="" NODE_PID=""
cleanup() {
  [ -n "$NODE_PID" ] && kill "$NODE_PID" 2>/dev/null || true
  [ -n "$STORE_PID" ] && kill "$STORE_PID" 2>/dev/null || true
  wait 2>/dev/null || true
}
trap cleanup EXIT

echo "== us-s3gw on 127.0.0.1:$T42_PORT (root $WORK/root)"
RUST_LOG="${LIVE_RUST_LOG:-us_s3gw=debug}" "$BIN" --host 127.0.0.1 --port "$T42_PORT" \
  --access-key "$T42_ACCESS" --secret-key "$T42_SECRET" "$WORK/root" >"$WORK/store.log" 2>&1 &
STORE_PID=$!
for _ in $(seq 1 100); do
  (exec 3<>"/dev/tcp/127.0.0.1/$T42_PORT") 2>/dev/null && { exec 3>&- 3<&-; break; }
  kill -0 "$STORE_PID" 2>/dev/null || { cat "$WORK/store.log" >&2; t42_die "us-s3gw exited"; }
  sleep 0.1
done

echo "== celld deploy scripts/live-app"
"$CELLD" deploy "$APP" --bucket "s3://$T42_BUCKET" \
  --endpoint "http://127.0.0.1:$T42_PORT" >"$WORK/deploy.log" 2>&1 \
  || { cat "$WORK/deploy.log" >&2; t42_die "celld deploy failed"; }

echo "== celld node on 127.0.0.1:$NODE_PORT"
"$CELLD" --bucket "s3://$T42_BUCKET" --endpoint "http://127.0.0.1:$T42_PORT" \
  --listen "127.0.0.1:$NODE_PORT" >"$WORK/node.log" 2>&1 &
NODE_PID=$!
ready=""
for _ in $(seq 1 60); do
  st=$(curl -s -o /dev/null -w '%{http_code}' --max-time 2 "http://127.0.0.1:$NODE_PORT/status" 2>/dev/null || true)
  [ "$st" = "200" ] && { ready=1; break; }
  sleep 1
done
[ -n "$ready" ] || { tail -20 "$WORK/node.log" >&2; t42_die "celld node did not become ready"; }

echo "== write load: fill an ~8 MiB cell, then keep it dirty until L1 compaction"
python3 - "$NODE_PORT" <<'PY'
import os, sys, time, urllib.request

port = sys.argv[1]
CHUNK = 122880  # 120 KiB (under the 128 KiB per-value cap)

def put(key, chunk):
    req = urllib.request.Request(f"http://127.0.0.1:{port}/?k={key}", data=chunk, method="POST")
    with urllib.request.urlopen(req, timeout=20) as r:
        return r.status

t0 = time.time(); ok = fail = 0
data = os.urandom(CHUNK * 64)  # 64 distinct keys -> cell DB ~7.5 MiB
for i in range(64):
    try:
        ok += put(i, data[i * CHUNK:(i + 1) * CHUNK]) == 200
    except Exception as exc:  # noqa: BLE001
        fail += 1
        print(f"fill err {i}: {exc}", file=sys.stderr)
print(f"fill:   ok={ok} fail={fail} elapsed_s={time.time() - t0:.1f}")

t0 = time.time(); ok = fail = 0
for i in range(320):  # dirty the same pages: pending bytes / txids trigger L1
    try:
        ok += put(i % 64, os.urandom(CHUNK)) == 200
    except Exception as exc:  # noqa: BLE001
        fail += 1
        print(f"rotate err {i}: {exc}", file=sys.stderr)
print(f"rotate: ok={ok} fail={fail} elapsed_s={time.time() - t0:.1f}")
if fail:
    sys.exit(1)
PY

echo "== waiting for sync + compaction"
sleep 15

echo
echo "== L1 compaction rounds (celld node log)"
grep "ltx_compaction" "$WORK/node.log" | tail -5 \
  | grep -oE 'min_txid=[0-9]+ max_txid=[0-9]+|input_bytes=[0-9]+|output_bytes=[0-9]+' || true

echo
echo "== LTX object census (bytes)"
find "$WORK/root/$T42_BUCKET/cells" -name '*.ltx' -printf '%s\n' 2>/dev/null | sort -n | awk '
  { a[NR] = $1; if ($1 >= 5242880) big++; if ($1 < 1024) b1++; else if ($1 < 600000) b2++; else if ($1 < 5242880) b3++; else b4++ }
  END {
    if (NR == 0) { print "no LTX objects"; exit 1 }
    printf "count=%d min=%d median=%d max=%d\n", NR, a[1], a[int(NR / 2)], a[NR]
    printf "bands: <1KiB=%d  1K..600K=%d  600K..5M=%d  >=5M=%d\n", b1 + 0, b2 + 0, b3 + 0, big + 0
  }'
MAX=$(find "$WORK/root/$T42_BUCKET/cells" -name '*.ltx' -printf '%s\n' 2>/dev/null | sort -n | tail -1)
echo "largest objects:"
find "$WORK/root/$T42_BUCKET/cells" -name '*.ltx' -size +4M -printf '  %s %p\n' 2>/dev/null | sort -rn || true

echo
echo "== multipart uploads served by us-s3gw (store debug log)"
MP=$(grep -c "multipart etag" "$WORK/store.log" || true)
grep "multipart etag" "$WORK/store.log" | tail -4 || true

echo
if [ "$MP" -ge 1 ] && [ "${MAX:-0}" -ge 5242880 ]; then
  echo "PASS: real celld stored a >=5 MiB object ($MAX B) through us-s3gw multipart ($MP multipart uploads)"
  echo "work dir kept for inspection: $WORK"
  exit 0
fi
echo "FAIL: expected a >=5 MiB multipart object (max=$MP multipart uploads, max object=${MAX:-0})" >&2
echo "work dir kept for inspection: $WORK" >&2
exit 1
