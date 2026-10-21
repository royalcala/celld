# PR-ready summary

**Title:** RPC cross-isolate completeness, durable fleet secrets, and
observability

**Branch:** `celld-os/rpc` (based on celld `main`; no PR has been opened,
#174 included).

## What this branch delivers

- **Cross-isolate RPC**: typed capability tokens, streams (including BYOB),
  facet and deployed-script facets, durable `ctx.restore`, typed errors,
  mid-flight cancellation, and capability routing across nodes.
- **Ownership/failover**: cross-node routing under the ownership fence, hard
  kill takeover of cells and durable handles, strict single-owner behavior on
  restart, and a documented mixed-version/rolling window.
- **Fleet secrets**: config names only; deploy supplies values from files;
  AES-256-GCM at rest with `CELLD_SECRET_KEY` (never in the bucket);
  `secret_text`/`secret_key` injection at load; redaction in logs and errors;
  per-deployment rotation.
- **Observability**: end-to-end `x-celld-request-id`, `cloudflare:workers`
  `tracing` spans backed by bounded host records, a low-cardinality metrics
  registry with `/metrics` on the operator listener, redacted/capped logs.
- **Tests/tooling**: `tools/s3gw` vendored (conditional writes under one CAS
  lock, hashed metadata filenames so long facet keys fit), fixtures and
  scripts, CI jobs `check`, `two-node`, `secrets`.

## Commits (oldest → newest)

```
674a9c4 feat(rpc): complete single-node capability lifecycle
ef5e87c feat(rpc): carry streams across isolates as capabilities
2bd84cd feat(rpc): durable ctx.restore persistent stubs
5dfd6be feat(rpc): typed capability errors and persistent stub storage
1995b37 feat(rpc): facet-owned persistent stubs through the parent chain
661efcd ci: gate the runtime on format, clippy, tests and a binary build
66d8688 feat(rpc): propagate caller cancellation into the callee event
56a669c docs(examples): runtime evidence for persistent stubs and cancellation
bb1e6db feat(rpc): host facet classes from co-hosted scripts
e599508 feat(rpc): route capabilities and cancellation across nodes
d477737 feat(rpc): bounded mixed-version compatibility and fleet evidence
54fa57a test: vendor us-s3gw as the fleet's local S3 endpoint
9e34aff feat(secrets): encrypted fleet secrets with deploy-only values
8c45744 feat(observability): request ids, tracing spans and a metrics endpoint
ede7583 fix(observability): commit the request-id/tracing harness
```

## Tests and evidence

- `cargo +1.98.1 test --workspace`: 36 tests (capabilities, streams/refcounts,
  secrets/crypto/redaction, metrics, spans).
- `cargo +1.98.1 fmt --all -- --check` and
  `cargo +1.98.1 clippy --workspace --all-targets -- -D warnings`: clean.
- `scripts/e2e-2node.sh` (on `tools/s3gw`): cross-node capability, BYOB
  stream, durable restore, stateless typed rejection, cancellation, hard-kill
  takeover, strict single owner from the bucket `own.json`, restart.
- `scripts/e2e-mixed-version.sh`: v1 durable accepted, v1 transient
  `forbidden`, v2-on-old `forbidden`, upgraded node converges, owner stable.
- `scripts/e2e-secrets.sh`: no plaintext in deploy output/bucket/logs,
  right/wrong/missing key, request-id echo, span logs with a redacted secret
  attribute, RPC/stream paths, `/metrics`, rotation (new version + served).
- celld-os app-level tests (manager’s uncommitted diff):
  `celld-2node.test.ts` (cross-node + typed `owner-moved` after takeover) and
  `celld-2node-hook.test.ts` (scheduler hook fires after a hard node kill;
  restart does not double-fire).  The latter passed 1 test in 65s on the
  final runtime.
- CI: `check`, `two-node`, `secrets`.

## Rollout

Follow `docs/rolling-deploy.md` and `docs/SECRETS.md`.  Upgrade every node to
a C build before the first secrets deployment (a node predating the
required-features check cannot refuse `secrets-v1`).  Rotate a secret per
deployment; rotate `CELLD_SECRET_KEY` by redeploying each secret under the new
key.  Drain a node before changing its binary; transient capabilities are
re-acquired, durable handles ride the bounded compat path.

## Known limits

In-flight RPC/stream at the instant of a hard kill is ambiguous and not
replayed; disposing a foreign capability does not send a remote release (the
owner entry is reclaimed with its cell/isolate); the operator `/metrics` has no
external auth/scrape policy; nested deployed-script facets are limited to the
gatekeeper/gadget parent-chain shape.

## Review focus

1. `crates/celld/capability.rs` (token fields/validator, v1 durable-only
   compat) and `docs/rolling-deploy.md`.
2. `crates/celld/secrets.rs` + `deploy.rs` (AAD/version/feature, no plaintext
   in manifest/output) and `docs/SECRETS.md`.
3. `crates/celld/js/harness.js` + `observability.rs` (request id, tracing
   bounds/redaction, metrics cardinality).
4. `main.rs` routing/facet paths and the E2E scripts.

No PR (#174) was opened from this branch.
