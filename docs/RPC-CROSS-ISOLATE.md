# RPC cross-isolate architecture

Status: implemented on `celld-os/rpc` (gates A, B and C).  Phase 2 / the HTTP
gateway are out of scope for this document.

## Scope

celld runs every isolate in a pool slot and isolates communicate through
host-mediated capabilities, not shared memory.  This document describes how a
capability crosses isolate, cell, facet, worker and node boundaries, how the
ownership fence degrades after a move, and what the runtime guarantees for
streams, durable restore, cancellation, secrets and observability.

Non-goals: a new gateway, service-binding protocol v2, or cross-fleet tracing
export.

## Invariants

- JavaScript never receives a raw handle, owner scope, heap id or capability
  secret.  It sees an opaque proxy; every operation is a host call.
- A token is authenticated with the fleet key (HMAC).  The key is derived from
  the persisted peer secret, so it is stable across node restarts and shared by
  nodes in the same fleet, and never written to the bucket.
- A **transient** handle belongs to one isolate incarnation.  A token from a
  dead node or retired isolate must fail typed; it must never resolve to a
  same-numbered handle on a new owner.
- A **durable** handle (`ctx.restore`) is re-resolved by scope after a move; the
  owner replays `[restore](params)` from its row.  It deliberately skips the
  isolate fence.
- A capability op is bounded by the current request/deadline and cannot walk
  outside the RPC receiver's visible surface.

## Token model

`CapabilityClaims` (`crates/celld/capability.rs`):

| field | meaning |
| --- | --- |
| `version` | `2` on the wire; `1` is parsed only by the durable-only compat path |
| `kind` | `cell` (owner scope) or `stateless` (owner isolate/heap) |
| `owner` | cell scope for cell/facet handles; empty for stateless |
| `node` | minting node name (transient fence) |
| `generation` | process incarnation × isolate heap id (transient fence) |
| `route` | facet handles: the published root scope used for routing |
| `handle` | per-owner handle id |
| `flags` | `1` callable stub, `2` durable restore, `4` facet restore |

Minting fills the node/incarnation at `__cap_mint` (`js.rs`) and the facet
route from `facet_root(owner)`.  `__restore_mint` mints durable handles with an
empty node/generation so a move does not invalidate them.

Owner-side validation runs inside the callee event in `__cap_check`:

- durable (`FLAG_RESTORE`): verify the MAC and that the owner scope equals the
  running cell storage; skip node/generation;
- transient: require the minting node to equal this node, the generation to
  equal `hash(process incarnation, current isolate heap)`, and the owner to
  equal the running cell storage;
- otherwise the caller sees a typed `owner-moved` / `stale-generation`.

A `ctx.exports` service stub, a `DurableObjectClass` and a deployed-script
facet class are **durable identity**, not transient handles: they cross as
script/entrypoint/class markers regardless of the capability-lifting flag.
Service stubs and their storage revival live in `js/harness.js`.

## Surfaces

- **Durable Object RPC** (`__rpc_call`) and **capability ops**
  (`__stub_rpc_call_token`, `__celldStubOp`) use the same cell dispatch.
- **WorkerEntrypoint / service bindings** go through `__svc_rpc`; the callee
  reply may carry service stubs, classes and durable tokens.
- **Facets** (`ctx.facets`) are embedded cells of a root.  Locally the facet
  root routes the op; remotely the token's `route` names the published root,
  the peer tunnel forwards the facet scope, and the owner dispatches the
  facet.  A facet class from a **deployed service** is hosted by the script
  loader (a runtime feature for co-hosted deployed scripts): the class marker carries the target
  script, `__script_loader(script)` opens one dedicated slot for that
  co-hosted script, and `ctx.facets.get` starts the facet there.  The facet
  storage still belongs to the root.
- **Streams** are lifted as capability tokens.  The owner keeps the bridge;
  the receiver reconstructs a **byte** stream, so BYOB works even when the
  sender built a default stream.  Reads/writes/cancel/close are bounded
  owner ops.  Transfer moves the one reference; disposal releases it and the
  last release disposes the bridge.  A disposed or moved owner fails typed.
- **Durable restore** stores `{spec|parent, params}` in the cell's
  `_cf_RESTORE` table, keyed by a durable handle.  Disposal deletes unused
  rows; stored child rows keep a parent reference alive.  `[restore]` is
  replayed once per residency and cached; cell release drops the cache.

## Routing, ownership and failover

`dispatch_rpc_call` (`main.rs`):

1. validates the scope (and, for facet tokens, routes by the root while
   dispatching the facet scope);
2. resolves the route through the ownership store: `Local` drives the cell
   here, `Remote` forwards over the peer tunnel with the caller request id;
3. the peer tunnel server routes the forwarded scope through the same local
   path (including facets);
4. stale routes are retried/classified (`RemoteRouteRetry`); a fenced or moved
   owner becomes `owner-moved`, a dead transport `transport`, a gone cell
   `owner-gone`.

`CellJob::request_id` and the wake/lease machinery implement failover.  A
hard-killed owner's cells are re-adopted from the bucket after the lease TTL;
a transient token from the dead node/isolate is rejected; a durable handle is
replayed.  A restarted node must not steal a live lease; both public listeners
then route every call to the single current owner.

Alarm/wake takeover: a scheduled cell publishes a wake entry asynchronously.
A SIGKILL in the first moments after `setAlarm` can lose that entry; graceful
shutdown or a short settle avoids it (the app-level hook test waits five
seconds after `enableHook`).

## Cancellation

Each outbound cell RPC carries the caller request id.  Dropping the caller
host op (client disconnect, `ctx.abort()`, context end) publishes an abort;
the owner samples it between turns, fails the event and drops its ops.  A
non-yielding turn completes its current turn and the runtime stops waiting.
The caller-side select sends a peer abort for a forwarded RPC.  A cell RPC
in flight at the exact moment of a hard node death is ambiguous by design:
the caller gets `transport`/an ambiguous error and the call is not replayed.

## Secrets

Design: [`docs/SECRETS.md`](./SECRETS.md).  Summary: config declares names and
kinds only; deploy reads values from files, encrypts each with AES-256-GCM
using `CELLD_SECRET_KEY` (32 bytes, 64 hex, environment only), binds the
ciphertext to script/name/kind via AAD, uploads an envelope and puts only the
names/kinds + ciphertext hash in the manifest/version.  The node decrypts once
at deployment load, injects `secret_text`/`secret_key` bindings, and registers
the text values (and key encodings) with the process redactor.  Missing/wrong
key fails adoption with a clear, redacted error; the last good generation
keeps serving.  `secrets-v1` is a required deployment feature.  A node that
predates the required-features check cannot be forced to refuse: upgrade every
node to a C build before the first secrets deploy.

## Observability

- Request id: every fetch has `x-celld-request-id` (client-supplied or
  generated), echoed on the response, stored in the event ALS, and prefixed on
  console lines.  RPC/DO fetch dispatch opens the same frame.  `tracing.isTraced()`
  is true when the event has a request id; RPC/timer events can still open
  spans explicitly.
- `cloudflare:workers` exports `tracing`: `enterSpan` (sync/async),
  `getActiveSpan`, `Span.setAttribute(s)`, `Span.end`, `isTraced`.  Spans are
  bounded host records logged as `span_begin`/`span_end` with span id, parent,
  request id, redacted attributes and duration.
- Metrics (`crates/celld/observability.rs`) are low-cardinality counters plus
  duration sum/count: RPC calls/errors/latency by transport and code, typed
  host errors by code, HTTP requests, stream reads/writes/cancels, trace
  spans, ownership wake takeovers.  `/metrics` is served on the operator
  listener.
- Logs are redacted through the secrets redactor and capped (8 KiB); request
  bodies, headers, tokens and secrets are never logged by the runtime.

## Error taxonomy

`owner-gone`, `owner-moved`, `stale-generation`, `disposed`, `forbidden`,
`transport`, plus `method-not-found` / ordinary TypeErrors.  Host routing
failures are classified at the boundary; owner JS errors cross as typed
envelopes.  `onRpcBroken` uses `error.code`, never message regexes.

## Compatibility and rollout

- Token v2: `docs/rolling-deploy.md` has the mixed-version matrix and the
  drain/upgrade procedure.  v1 durable works through the bounded compat path;
  v1 transient and v2-on-old are typed failures.
- Required features gate new manifest semantics (`secrets-v1`, assets, R2,
  queues, python).  A C+ node refuses an unknown feature; a pre-feature node
  cannot be protected by a feature it does not know.
- Secrets rotation is per deployment; a new value is a new version.  Master
  key rotation requires redeploying each secret with the new key (values come
  from the secret manager, never from the bucket).

## Limits / known issues

- In-flight RPC/stream at the instant of a hard node death is ambiguous; it is
  not replayed (a retry could duplicate a side effect).
- Disposing a foreign capability on a receiving node does not currently send a
  remote release; the owner entry is reclaimed with its cell/isolate.  Durable
  rows are irrevocable in the stored window.
- Nested facets across deployed scripts are limited to the parent-chain shape
  used by gatekeeper/gadget facets.
- The operator `/metrics` endpoint is internal-listener only; no external
  scraping/auth policy is defined yet.
- The pre-C required-features gap above.

## Evidence and CI

- `cargo test`: 36 unit tests (capabilities, secrets/crypto/redaction,
  metrics, spans).
- `scripts/e2e-2node.sh`: cross-node capability, BYOB stream, durable
  restore, stateless rejection, cancellation, hard kill takeover, strict
  single owner (bucket `own.json`), restart.
- `scripts/e2e-mixed-version.sh`: v1 durable accept, v1 transient reject,
  v2-on-old reject, upgrade convergence.
- `scripts/e2e-secrets.sh`: encrypted deploy, no plaintext in bucket/logs,
  right/wrong/missing key, request id, span + secret attribute redaction,
  RPC/stream paths, `/metrics`, rotation.
- `tools/s3gw` acceptance (`celld diagnose` + the CAS race) and CI jobs:
  `check`, `two-node`, `secrets`.
- The celld-os app-level 2-node tests are the manager's uncommitted diff.

## Fork and PR

Fork policy: [`docs/FORK.md`](./FORK.md).  PR-ready summary:
[`docs/PR-READY.md`](./PR-READY.md).  No PR (#174) has been opened.
