# Runtime release candidates

This repository is the runtime source of truth; the operational mirror lives
in `celld-os`. Release candidates are cut from the `candidate` branch: a push
there makes `.github/workflows/release.yml` build every native target and open
a **draft** `v<version>` release. Publishing the draft is a separate,
human-approved step.

## v0.6.3 (candidate)

Dynamic Worker Tail Workers now receive an event for an **entrypoint RPC**
invocation, not only for `fetch`. A code-execution tool that calls `run()` over
RPC used to wait forever for logs that celld never delivered; its tail now
carries the console output, exceptions, and outcome.

- Tails fix: `cff193c9e085fc6c81f038bbad717bea0d703fa3`
  (`fix(loader): deliver Tail Worker events for entrypoint RPC`).
- Tail events keep workerd's shape: a fetch carries `request`/`response`
  metadata, an RPC carries `event.rpcMethod` (so a caller can ignore its
  `verify()` probe). Delivery stays on the caller's `waitUntil`, and a Tail
  Worker failure does not change the response.
- Candidate ref: `candidate` at the commit that contains this page. The
  release workflow creates the draft `v0.6.3` tag at that commit; this page
  does not pin its own hash.
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
