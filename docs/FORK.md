# Fork policy (celld → `celld-os/rpc`)

This branch carries the RPC cross-isolate work, fleet secrets and observability
on top of upstream celld.  Treat it as a **patch series over a pinned celld
release**, not as a merge of unrelated history.

## Branch and patch series

- Branch: `celld-os/rpc`.  Do not merge celld `main` into it; rebase.
- One atomic commit per concern (capabilities, streams, facets/script loader,
  durable restore, cancellation, rolling compatibility, secrets,
  observability, docs).  A commit must build and pass its tests on its own.
- Pin the toolchain the fork builds with (`RUST_VERSION: 1.98.1` in
  `.github/workflows/ci.yml` and the release workflow).  A Rust version change
  is its own review.
- `tools/s3gw` is vendored, excluded from the cargo workspace, and only used
  by tests/CI.  Keep its upstream source and the local patch (hashed metadata
  filenames for long facet keys) in the same commit; do not put test data or
  credentials under it.

## Rebase / upgrade procedure

1. Fetch the target celld release/tag; rebase the patch series on it (not a
   merge).
2. Resolve against the RPC/JS harness, storage and protocol changes first;
   keep each patch's tests with the patch.
3. Run locally/CI: `cargo fmt --all -- --check`,
   `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`, the `two-node` job, the `secrets` job, and the
   celld-os E2E (the manager owns the latter).
4. If a rebase changes the capability token layout, the manifest required
   features or the wire protocol, update `docs/rolling-deploy.md` and the
   mixed-version script **in the same patch**: old nodes must fail typed, never
   silently lose a fence.

## Compatibility rules

- Never accept a transient legacy token without a node/incarnation fence just
  to make a rollout easier.  The bounded compat path is durable-only
  (`FLAG_RESTORE`).
- New manifest semantics get a required feature (`secrets-v1`, …).  A node
  that predates the required-features check cannot be forced to refuse; state
  that in the rollout notes and upgrade the fleet before deploying the new
  semantics.
- Keep the token version bump and the mixed-version tests in the same patch as
  the layout change.
- Secrets: values never enter the config, the repository, the manifest or a
  log.  Crypto/AAD/redaction changes need focused unit tests plus the secrets
  E2E; never weaken the AAD binding or the recovery-key handling.

## Upstream proposal criteria

Propose a patch upstream when it is generally useful and self-contained:

- new capability claims fields/routing, facet hosting for deployed scripts,
  service-stub/class crossing, RPC cancellation, the tracing/metrics surface;
- a bug fix with a deterministic test;
- no celld-os-specific policy or product config.

Keep in the fork: celld-os deployment bindings, the test gateway choice, and
anything that only exists to serve this product.  An upstream PR must include
the cross-isolate/2-node/mixed-version evidence it relies on; do not send a
patch that changes token semantics without the compatibility plan.

## Review and CI

- CI must be green before a patch lands: `check`, `two-node`, `secrets` jobs.
- The two-node and secrets jobs use `tools/s3gw` (conditional writes under one
  CAS lock, hashed metadata names).  Do not replace it with an endpoint that
  accepts `If-Match` without enforcing it (Garage) or cannot store the long
  facet keys (s3s-fs).
- Observability changes must keep bodies, headers, tokens and secrets out of
  logs and metrics; labels stay low-cardinality.
- No PR (#174) is opened from this branch; the manager owns integration into
  celld-os and the PR description.  Keep `docs/PR-READY.md` current as the
  branch summary.
