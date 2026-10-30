# Rolling deploy across a mixed-version fleet

celld capability tokens are fleet-keyed.  A version change that alters the
token layout (for example, the node / isolate-incarnation / facet-route fields
added for cross-node routing) therefore requires a fleet procedure, not a
per-node restart.

## Compatibility window

The current build accepts the previous token layout for **durable handles
only** (`FLAG_RESTORE`).  A durable `ctx.restore` handle keeps working when its
cell moves to an upgraded node: the row is re-resolved there and the isolate
fence is not needed.  Everything else is rejected with a typed error:

| Token minted by | Used on | Result |
| --- | --- | --- |
| previous build, durable handle | current build | accepted (compat path) |
| previous build, transient capability | current build | `forbidden` (`legacy capability has no cross-node fence`) |
| current build | previous build | `forbidden` (previous build rejects the new version) |
| current build | current build | normal |

Transient capabilities are connection-scoped; re-acquire them after the fleet
is upgraded.  The compat path deliberately does not accept transient legacy
tokens, because they carry no node or isolate incarnation and could alias a
reused handle.

## Persisted session tokens

`capabilityToken(stub)` / `restoreCapability(token)` (both from
`cloudflare:workers`) let an application store a transient capability as its
token and revive it later — the shape a Durable Object needs to re-arm a
subscription that lives in a browser session.  Under a rolling upgrade:

- A stored token is a normal current-version transient token, so it is fenced
  exactly like a live one: it survives a node restart (the key is derived from
  the persisted peer secret and the owner is a session isolate that outlives the
  object), and it fails typed the moment the owner's isolate incarnation is
  gone.
- A build that predates the surface has no `capabilityToken` /
  `restoreCapability` export: in a bundle importing either the name is
  `undefined`, so the first call throws instead of storing or reviving anything.
  Upgrade every node that may host the deployment **before** deploying such a
  bundle — the same rule as any other new surface.  It is deliberately not a
  config-driven deployment feature: nothing in the manifest describes an
  application's import of a module export, so a required-feature gate could not
  be derived from the deployment.
- The stored token itself stays a current-version token, so the matrix above is
  unchanged: a later token-layout change reaches persisted tokens with the same
  `forbidden` / typed behaviour as live ones, and a stored token is never a
  separate compatibility path.

## Local / CI bucket endpoint

Use `tools/s3gw` (vendored, `us-s3gw`). It is the small `s3s`-based store with
the conditional check+write serialized under one CAS lock, which is what the
ownership fence needs: `scripts/accept.sh` runs `celld diagnose` and
`scripts/race.sh` asserts exactly one winner per concurrent `If-None-Match`
round. It also hashes metadata filenames, so celld's long facet-log keys fit
`NAME_MAX` (the reason s3s-fs crops up as `File name too long` here). Build it
from `tools/s3gw`; the fleet scripts take any S3 endpoint (`S3_ENDPOINT`,
`S3_BUCKET`, and `S3_LOCAL_ROOT` for the ownership record). Garage accepts
`If-Match` without enforcing it; MinIO community artifacts are no longer
downloadable. `S3_LOCAL_ROOT` is only used for the local filesystem layout
(s3gw/s3s-fs); with a network store read the ownership record through the S3
API instead.

## Procedure

1. `celld diagnose --bucket … --endpoint …` on every bucket endpoint; the
   conditional-write check must pass.
2. Upgrade one node at a time.  Before upgrading a node, drain it: stop sending
   it new public traffic and let in-flight calls finish.  Its cells are handed
   off (graceful shutdown) or taken over after the lease TTL.
3. Replace the binary and start the node.  Verify:
   - `celld diagnose …` passes;
   - the public API answers;
   - the cell ownership record in the bucket (`cells/<scope>/own.json`) names a
     single live node for the cells you touched.
4. Do not route traffic to an un-upgraded node for capabilities minted by an
   upgraded node: the old build rejects the new token with `forbidden`.  Drain
   old nodes before new ones start minting into them.
5. After the whole fleet is upgraded, re-establish transient capabilities
   (connections re-acquire them).  Durable hooks/stubs keep their handles
   through the compat window.

## Verified window

`scripts/e2e-mixed-version.sh` (pre-B node A + current node B, then A upgraded)
prints:

- `v1 durable handle stored on A, used on the new node B` — accepted;
- `v1 transient handle from A rejected typed on B` — `forbidden`;
- `v2 transient handle from B rejected by the pre-B node A` — `forbidden`;
- after upgrading A, a fresh callback works (`pong`) and the durable handle
  still reads;
- the cell ownership stays with node B throughout the upgrade.

## Rollback

The previous binary rejects the current token version with a typed
`forbidden`.  Drain the nodes before rolling back, then re-establish transient
capabilities and restart durable handlers; the durable store (restore rows,
DO storage) is not modified by the token version.

## Hard-kill alarm window

A scheduled/alarm cell publishes its wake entry asynchronously.  A node that is
SIGKILLed in the first moments after enabling an alarm can lose that wake
entry, and no successor has a record to fire.  Allow the alarm to ship (a few
seconds) before hard-killing the owner; graceful shutdown drains it.  The
2-node app hook test waits five seconds after `enableHook` for this reason.

## Encrypted secrets

- The config lists **names and kinds only**:
  `"secrets": {"API_TOKEN": "text", "TLS_KEY": "key"}`.
- Values never come from the config. Pass them at deploy time:
  `--secret API_TOKEN=@token.txt` (or `--secret-file API_TOKEN=token.txt`;
  `@-` reads one value from stdin). Literal values are refused, and the deploy
  output/`--json`/errors never echo a value.
- `CELLD_SECRET_KEY` (32 bytes, 64 hex) must be in the deploy and node
  environments. It is never written to the bucket, the manifest or a log.
  Values are AES-256-GCM encrypted with an AAD binding script + name + kind.
- The manifest carries names/kinds and the ciphertext SHA-256, never a value;
  the version changes with the ciphertext, so rotating a value is a new
  deployment generation. The node decrypts once at load, injects
  `secret_text`/`secret_key` bindings, and registers the values with the log
  redactor.
- Missing/wrong key: the node rejects that deployment with a clear, redacted
  error and the previous generation keeps serving (a node with no previous
  generation does not serve it).
- `secrets-v1` is a required deployment feature. **Upgrade every node to a C
  build before the first secrets deploy**: a binary that predates the
  required-features check cannot be forced to refuse and may serve missing
  bindings. The `e2e-secrets.sh` pre-C probe is informational for that reason.
- Rotation: deploy a new value (new version); nodes adopt it. Master-key
  rotation: redeploy each secret with the new `CELLD_SECRET_KEY` (values come
  from the secret manager, never from the bucket).

## Observability

- Every fetch gets `x-celld-request-id` (client-supplied or generated) and
  echoes it on the response; it is in the event ALS, the `tracing` span
  records and the console-line prefix. RPC/DO fetch dispatch opens the same
  request frame.
- `cloudflare:workers` exports `tracing` (`enterSpan`, `getActiveSpan`,
  `Span.setAttribute(s)`, `Span.end`, `isTraced`). Spans are bounded host
  records (`span_begin`/`span_end`) with a parent, request id, redacted
  attributes and duration. A secret used as an attribute appears as
  `[redacted]`.
- Metrics are low-cardinality: RPC calls/errors/latency by transport/code,
  typed host errors, HTTP requests, stream reads/writes/cancels, trace spans,
  ownership wake takeovers. `curl http://127.0.0.1:<internal>/metrics` on the
  operator listener. No bodies, headers, tokens or secrets.
- Console lines are redacted and capped (8 KiB).

## Final C checklist

Local/CI (any S3 endpoint; `tools/s3gw` is the reference):

```sh
cargo +1.98.1 fmt --all -- --check
cargo +1.98.1 clippy --workspace --all-targets -- -D warnings
cargo +1.98.1 test --workspace

S3_ENDPOINT=... S3_BUCKET=... S3_LOCAL_ROOT=... \
  bash scripts/e2e-2node.sh
S3_ENDPOINT=... S3_BUCKET=... S3_LOCAL_ROOT=... PREB_BIN=... \
  bash scripts/e2e-mixed-version.sh
S3_ENDPOINT=... S3_BUCKET=... S3_LOCAL_ROOT=... \
  CELLD_ESBUILD="$(command -v esbuild)" bash scripts/e2e-secrets.sh
```

Checks:

- `tools/s3gw/scripts/accept.sh` (celld diagnose: conditional create/update)
  and `race.sh` (exactly one winner per concurrent `If-None-Match` round).
- Ownership record: one `own.json` node per cell; before kill A -> A, after
  takeover -> B, after A restarts -> B (no steal). Calls through A and B work.
- No plaintext secret in the deploy output, bucket object tree or node logs;
  the span log shows the secret attribute redacted.
- The app-level celld-os tests (manager diff): cross-node + typed
  `owner-moved`, and the scheduler hook firing after a hard node kill with no
  double fire after restart.
