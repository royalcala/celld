# Upstream sync runbook

The runtime is a patch series over upstream `denoland/celld`. This page is the
repeating procedure for taking a new upstream release; `docs/FORK.md` defines
the branch, commit and compatibility policy. Never merge into `main` and never
force-push a shared branch.

## Remotes and names

| remote | URL | role |
| --- | --- | --- |
| `origin` | `https://github.com/royalcala/celld.git` | fork: `celld-os/rpc`, feature branches, RC tags |
| `upstream` | `https://github.com/denoland/celld.git` | source of celld releases |

Keep the integration branch `celld-os/rpc`, the RC trigger `candidate`, and
short-lived `upstream-sync/vX.Y.Z` branches.

## Versions and tags

Upstream tags are `v<upstream-version>`. A fork candidate must not reuse an
upstream tag, and an existing fork tag is immutable. Use a celld suffix:

- Cargo version: `X.Y.Z-celld.N` (for example `0.6.4-celld.1`)
- Tag and draft release: `vX.Y.Z-celld.N`
- `candidate` is pushed at the RC commit; the release workflow builds and
  opens the **draft** release. Publishing is a separate human step.

`v0.6.3-rc.1` already exists at `1120c81` and must not be moved or reused.

## Procedure

0. **Preconditions.** Clean working tree, integration branch gates green, and
   record the rollback ref:
   ```bash
   git rev-parse celld-os/rpc
   ```
1. **Fetch tags, not just branches.** Tags are the release source of truth.
   ```bash
   git fetch origin --tags
   git fetch upstream --tags
   git rev-parse 'vX.Y.Z^{}' upstream/main
   ```
   Use the tag commit; a moving upstream branch is not a release.
2. **Inventory the delta.**
   ```bash
   base=$(git merge-base celld-os/rpc vX.Y.Z)
   git log --oneline "$base"..vX.Y.Z
   git diff --stat "$base"..vX.Y.Z
   git diff "$base"..vX.Y.Z -- crates docs Cargo.toml
   ```
   Record the base, the range and the files in the sync log below. Carry every
   security/dependency floor and every ownership/durability fix unless a
   documented reason says otherwise.
3. **Branch from the integration tip.**
   ```bash
   git switch -c upstream-sync/vX.Y.Z celld-os/rpc
   ```
4. **Merge by default.**
   ```bash
   git merge --no-ff --no-edit vX.Y.Z \
     -m "merge: upstream celld vX.Y.Z"
   ```
   A rebase is acceptable only when the branch has not been pushed or shared
   and the conflicts are mechanical; write the reason in the sync log.
5. **Resolve conflicts in this order.**
   - Keep the feature series: A1 tokens/security/lifecycle, A2 targets/facets,
     A3 streams, A4 durable `ctx.restore`, A5 E2E/CI; C4/P3 (metrics, `cpu_ms`
     spans, dev profiles); tails (fetch **and** RPC Tail Worker delivery).
   - Take upstream security, dependency and ownership/durability fixes even
     when they touch the same files; if one contradicts a feature, stop and
     document instead of silently dropping either side.
   - Resolve per file, then run the focused test. Never delete a feature test
     to make a merge pass.
   - `git diff --check`; no markers; `cargo fmt` before the final gates.
6. **Run the full matrix.**
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   cargo test -p celld --test dynamic_worker_tails   # tails: fetch + RPC
   cargo build --bin celld
   ```
   Fleet gates use the vendored gateway:
   ```bash
   cargo build --release --manifest-path tools/s3gw/Cargo.toml
   mkdir -p /tmp/s3gw-data/celld-fleet
   tools/s3gw/target/release/us-s3gw --host 127.0.0.1 --port 9000 \
     --access-key minioadmin --secret-key minioadmin /tmp/s3gw-data &
   # wait for http://127.0.0.1:9000 to answer

   S3_ENDPOINT=http://127.0.0.1:9000 S3_BUCKET=celld-fleet \
     S3_LOCAL_ROOT=/tmp/s3gw-data CELLD_BIN=target/debug/celld \
     CELLD_ESBUILD="$(command -v esbuild)" \
     bash scripts/e2e-2node.sh

   S3_ENDPOINT=http://127.0.0.1:9000 S3_BUCKET=celld-fleet \
     S3_LOCAL_ROOT=/tmp/s3gw-data CELLD_BIN=target/debug/celld \
     CELLD_ESBUILD="$(command -v esbuild)" \
     bash scripts/e2e-secrets.sh
   ```
   Run the fleet scripts against **separate buckets** (or a fresh s3gw root).
   `e2e-secrets.sh` inspects the first `manifest.json` under the bucket, so a
   bucket shared with the two-node fixture can make it read the wrong
   manifest.
   There is no separate tails fleet script: `dynamic_worker_tails` is its
   gate. Add new E2E scripts here when they land.
7. **Version, docs and tag.**
   - Set `crates/celld/Cargo.toml` to `X.Y.Z-celld.N`; run `cargo check` so
     `Cargo.lock` follows.
   - Update `docs/RUNTIME.md` (candidate record and exact hashes),
     `docs/PR-READY.md`, the pinned release in `docs/FORK.md`, and this log.
   - Commit, push `upstream-sync/vX.Y.Z`, then fast-forward `celld-os/rpc`.
   - Push `celld-os/rpc` to `candidate`; wait for the draft build.
   - Create the annotated tag `vX.Y.Z-celld.N` at the RC commit and push it.
     Never move a tag; cut a new `-celld.N+1` instead.
8. **Docs/pin update outside this repository.** The celld-os manager updates
   the runtime pin, its mirrored `docs/RUNTIME.md`, and the `1us-v4`/infra
   pins to the new tag and hash. Do not edit celld-os from a runtime sync.
9. **Rollback.**
   - Keep the previous tag and container digest. Unpublish or delete only a
     **draft** release; never rewrite a published tag.
   - On the integration branch, `git revert -m 1 <merge>` and cut a new
     candidate; do not reset a shared branch.
   - Redeploy the previous tag/digest. Mixed-version windows are bounded by
     `docs/rolling-deploy.md`; upgrade the fleet, not one node.
10. **When to upstream a patch.** Propose it when it is generally useful,
    self-contained, has a deterministic test, and carries no celld-os policy or
    product config. The RPC Tail Worker delivery (`cff193c`) is such a
    candidate. Keep product bindings, the test gateway choice, and fork-only
    configuration here.

## Sync log

| date | upstream | upstream commit | sync base | merge | conflicts | gates | candidate |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 2026-10-23 | `v0.6.3` | `8702f6d` | `90b4301` | `8ed6bcf` | none | fmt, workspace clippy, workspace tests, `dynamic_worker_tails` PASS, two-node PASS, secrets PASS (fresh bucket) | existing `v0.6.3-rc.1` (`1120c81`) untouched; no new tag |
