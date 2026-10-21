# shellcheck shell=bash
# Shared helpers for the us-s3gw verification scripts.
# Sourced by accept.sh / race.sh / bulk.sh; not executable on its own.

T42_SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
T42_REPO_ROOT=$(cd "$T42_SCRIPT_DIR/.." && pwd)

T42_PORT="${S3GW_PORT:-9020}"
T42_BUCKET="${S3GW_BUCKET:-fleet}"
T42_ACCESS="${S3GW_ACCESS_KEY:-t42-test-access}"
T42_SECRET="${S3GW_SECRET_KEY:-t42-test-secret}"
T42_REGION="${S3GW_REGION:-us-east-1}"

t42_die() {
  echo "ERROR: $*" >&2
  exit 1
}

# Verification against an already-running store (e.g. the systemd service):
# set S3GW_ENDPOINT=http://127.0.0.1:9020 to skip starting a private one.
t42_store_endpoint() {
  printf '%s' "${S3GW_ENDPOINT:-http://127.0.0.1:$T42_PORT}"
}

t42_external_store() {
  [ -n "${S3GW_ENDPOINT:-}" ]
}

t42_store_bin() {
  if [ -n "${S3GW_BIN:-}" ]; then
    printf '%s' "$S3GW_BIN"
    return 0
  fi
  local cand="$T42_REPO_ROOT/target/release/us-s3gw"
  if [ -x "$cand" ]; then
    printf '%s' "$cand"
    return 0
  fi
  return 1
}

t42_ensure_store_bin() {
  local bin
  if bin=$(t42_store_bin); then
    printf '%s' "$bin"
    return 0
  fi
  echo "us-s3gw binary not found, building (cargo build --release)..." >&2
  (cd "$T42_REPO_ROOT" && cargo build --release) >&2
  t42_store_bin || t42_die "cargo build --release did not produce target/release/us-s3gw"
}

# Locate a celld binary: $CELLD_BIN, PATH, then <repo>/celld/target/{release,debug}/celld
# walking up from this script (covers both the repo and a .worktrees/ checkout).
t42_find_celld() {
  if [ -n "${CELLD_BIN:-}" ]; then
    printf '%s' "$CELLD_BIN"
    return 0
  fi
  if command -v celld >/dev/null 2>&1; then
    command -v celld
    return 0
  fi
  local d="$T42_SCRIPT_DIR" i prof
  for i in 1 2 3 4 5 6; do
    d=$(dirname "$d")
    for prof in release debug; do
      if [ -x "$d/celld/target/$prof/celld" ]; then
        printf '%s' "$d/celld/target/$prof/celld"
        return 0
      fi
    done
  done
  return 1
}

t42_expect_celld() {
  local bin
  if bin=$(t42_find_celld); then
    printf '%s' "$bin"
    return 0
  fi
  t42_die "celld binary not found; set CELLD_BIN=/path/to/celld (needed for 'celld diagnose')"
}

# Start us-s3gw on 127.0.0.1:$T42_PORT with a throwaway root. Sets:
# T42_STORE_PID, T42_STORE_DATA, T42_STORE_LOG. Cleans up via t42_stop_store.
t42_start_store() {
  local bin
  bin=$(t42_ensure_store_bin)
  T42_STORE_DATA=$(mktemp -d "${TMPDIR:-/tmp}/us-s3gw-t42.XXXXXX")
  T42_STORE_LOG="$T42_STORE_DATA/server.log"
  if [ -n "${US_S3GW_NO_CAS_LOCK:-}" ]; then
    echo "WARNING: US_S3GW_NO_CAS_LOCK is set — the CAS lock is OFF (demo mode only)" >&2
  fi
  "$bin" --host 127.0.0.1 --port "$T42_PORT" \
    --access-key "$T42_ACCESS" --secret-key "$T42_SECRET" \
    "$T42_STORE_DATA" >"$T42_STORE_LOG" 2>&1 &
  T42_STORE_PID=$!
  local i
  for i in $(seq 1 100); do
    if (exec 3<>"/dev/tcp/127.0.0.1/$T42_PORT") 2>/dev/null; then
      exec 3>&- 3<&-
      return 0
    fi
    if ! kill -0 "$T42_STORE_PID" 2>/dev/null; then
      cat "$T42_STORE_LOG" >&2
      t42_die "us-s3gw exited during startup"
    fi
    sleep 0.1
  done
  t42_die "us-s3gw did not listen on 127.0.0.1:$T42_PORT within 10s"
}

# us-s3gw models a bucket as a directory under the root (create_bucket does
# exactly this), and `celld diagnose` validates the bucket with a list BEFORE
# probing it, so create it up front.
t42_ensure_bucket() {
  mkdir -p "$T42_STORE_DATA/$T42_BUCKET"
}

t42_stop_store() {
  if [ -n "${T42_STORE_PID:-}" ] && kill -0 "$T42_STORE_PID" 2>/dev/null; then
    kill "$T42_STORE_PID" 2>/dev/null || true
    wait "$T42_STORE_PID" 2>/dev/null || true
  fi
  if [ -n "${T42_STORE_DATA:-}" ] && [ -d "$T42_STORE_DATA" ]; then
    rm -rf "$T42_STORE_DATA"
  fi
  T42_STORE_PID=""
  T42_STORE_DATA=""
}
