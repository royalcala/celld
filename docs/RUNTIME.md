# Runtime release candidates

This repository is the runtime source of truth; the operational mirror lives
in `celld-os`. Upstream releases are taken through `upstream-sync/*` branches
(see [UPSTREAM-SYNC.md](UPSTREAM-SYNC.md)). A fork candidate is tagged
`v<upstream>-celld.<n>`; upstream tags are never reused or moved. The draft
release is built by pushing `candidate`; publishing is a separate,
human-approved step.

## v0.6.3-celld.1 (fork release candidate)

Upstream `v0.6.3` (`8702f6d`) is merged into the fork line (`1120c81`) at merge
commit `8ed6bcf`, with no conflicts. This is the fork candidate for that
upstream release.

- Tag: **`v0.6.3-celld.1`** (annotated, object
  `f846c3b3feac448820a7f79f77886aa80c07a129`) targeting
  `8e13f09355838ca58c6b8ee48b8c1355c5a49d3b`.
- Carries upstream's `rustls 0.23.46` / `rustls-webpki 0.103.16` /
  `webpki-roots 1.0.10` floors and the ownership-record loss refusal; keeps
  A1–A5 RPC, C4/P3 observability and dev profiles, and fetch+RPC Tail Worker
  delivery.
- Supersedes the pre-sync `v0.6.3-rc.1` (`1120c81`) for the fork line; that tag
  stays exactly as cut.
- Gates: `cargo fmt --check`; 47 unit tests plus `dev_profile` and
  `dynamic_worker_tails`; `cargo clippy --workspace --all-targets -- -D
  warnings`; `cargo test --workspace`; two-node and secrets E2E. The sync log
  in [UPSTREAM-SYNC.md](UPSTREAM-SYNC.md) records the evidence.

## v0.6.3 (pre-sync candidate)

This is the earlier fork candidate, kept as history. Dynamic Worker Tail
Workers now receive an event for an **entrypoint RPC** invocation, not only for
`fetch`. A code-execution tool that calls `run()` over RPC used to wait forever
for logs that celld never delivered; its tail now carries the console output,
exceptions, and outcome.

- Tails fix: `cff193c9e085fc6c81f038bbad717bea0d703fa3`
  (`fix(loader): deliver Tail Worker events for entrypoint RPC`).
- Tail events keep workerd's shape: a fetch carries `request`/`response`
  metadata, an RPC carries `event.rpcMethod` (so a caller can ignore its
  `verify()` probe). Delivery stays on the caller's `waitUntil`, and a Tail
  Worker failure does not change the response.
- Candidate ref: this was the `candidate` build at `1120c81`; the fork line is
  now `v0.6.3-celld.1`.
- Gates: `cargo fmt --check`; `cargo test -p celld` (47 unit tests plus the
  `dev_profile` and `dynamic_worker_tails` integration tests);
  `cargo clippy --workspace --all-targets -- -D warnings`, which the release
  container runs. Three pre-existing lint sites (`tokio::select!` in the
  WebSocket frame merge and two `nonminimal_bool` expressions) were repaired
  so the candidate image builds. The tails test loads a Dynamic Worker, calls
  `verify()` and `run()` over RPC, and asserts the caller receives
  `rpcMethod=run` with `hello from executeCode 42`, the object log line, and
  the `verify` event.

The full runtime history and the candidate branch are the authoritative
release record; this page records only what the candidate changed.
