---
id: CELLD-11
title: Enable Groma scanners and verify architecture view
status: Done
assignee: []
created_date: '2026-11-10T18:34:00Z'
updated_date: '2026-11-10T19:21:00Z'
labels: []
milestone: m-0
dependencies: []
references:
  - celld
  - celld-celld
  - celld-s3probe
  - us-s3gw
modified_files:
  - groma/config.json
  - groma/index.md
  - groma/systems/
  - groma/systems/celld/components/actor.md
  - groma/systems/celld/components/alarm-index.md
  - groma/systems/celld/components/alarm-websocket-worker.md
  - groma/systems/celld/components/alarm.md
  - groma/systems/celld/components/assets.md
  - groma/systems/celld/components/async-index.md
  - groma/systems/celld/components/asyncrt.md
  - groma/systems/celld/components/atob-btoa.md
  - groma/systems/celld/components/body-index.md
  - groma/systems/celld/components/bootstrap.md
  - groma/systems/celld/components/bucket.md
  - groma/systems/celld/components/byte-streams.md
  - groma/systems/celld/components/cancellation-index.md
  - groma/systems/celld/components/capability.md
  - groma/systems/celld/components/cell-cli.md
  - groma/systems/celld/components/cell-dispatch.md
  - groma/systems/celld/components/cell-host.md
  - groma/systems/celld/components/cell-runtime.md
  - groma/systems/celld/components/cell.md
  - groma/systems/celld/components/celld-container.md
  - groma/systems/celld/components/celld-crates-celld-js-crypto-887dcaf0.md
  - groma/systems/celld/components/celld-crates-celld-js-crypto-8d3fd54d.md
  - >-
    groma/systems/celld/components/celld-crates-celld-js-html-rewriter-6b05c950.md
  - >-
    groma/systems/celld/components/celld-crates-celld-js-html-rewriter-e3c2c7ec.md
  - groma/systems/celld/components/celld-crates-celld-python-worker-668a5ea2.md
  - groma/systems/celld/components/celld-crates-celld-python-worker-d03f3928.md
  - groma/systems/celld/components/celld-lib.md
  - groma/systems/celld/components/celld-wake.md
  - groma/systems/celld/components/checksum.md
  - groma/systems/celld/components/clean-reload.md
  - groma/systems/celld/components/cli-options.md
  - groma/systems/celld/components/cli-output.md
  - groma/systems/celld/components/client-bundle.md
  - groma/systems/celld/components/codec.md
  - groma/systems/celld/components/compaction-level.md
  - groma/systems/celld/components/compactor.md
  - groma/systems/celld/components/compression-streams.md
  - groma/systems/celld/components/container-index.md
  - groma/systems/celld/components/control-plane.md
  - groma/systems/celld/components/counter-index.md
  - groma/systems/celld/components/cron-index.md
  - groma/systems/celld/components/cron.md
  - groma/systems/celld/components/d1-cli.md
  - groma/systems/celld/components/d1-index.md
  - groma/systems/celld/components/db.md
  - groma/systems/celld/components/dead-node-gc.md
  - groma/systems/celld/components/dead-node-reconciliation.md
  - groma/systems/celld/components/deploy.md
  - groma/systems/celld/components/dev.md
  - groma/systems/celld/components/docker.md
  - groma/systems/celld/components/drain-token.md
  - groma/systems/celld/components/drain.md
  - groma/systems/celld/components/durability.md
  - groma/systems/celld/components/dynamic-worker-tails-index.md
  - groma/systems/celld/components/effects.md
  - groma/systems/celld/components/engine-api.md
  - groma/systems/celld/components/entry.md
  - groma/systems/celld/components/env-vars.md
  - groma/systems/celld/components/epoch-gc.md
  - groma/systems/celld/components/epochs.md
  - groma/systems/celld/components/event-source.md
  - groma/systems/celld/components/execute-code-tails-worker.md
  - groma/systems/celld/components/facet-streams.md
  - groma/systems/celld/components/facets-index.md
  - groma/systems/celld/components/fatal.md
  - groma/systems/celld/components/file.md
  - groma/systems/celld/components/fleet-fixture-index.md
  - groma/systems/celld/components/fleet.md
  - groma/systems/celld/components/format.md
  - groma/systems/celld/components/fs.md
  - groma/systems/celld/components/gate.md
  - groma/systems/celld/components/generation.md
  - groma/systems/celld/components/harness.md
  - groma/systems/celld/components/headers.md
  - groma/systems/celld/components/hello-index.md
  - groma/systems/celld/components/host-channels.md
  - groma/systems/celld/components/host-services.md
  - groma/systems/celld/components/host.md
  - groma/systems/celld/components/http-streams.md
  - groma/systems/celld/components/http.md
  - groma/systems/celld/components/identity-transform-stream.md
  - groma/systems/celld/components/input-gate-lifecycle.md
  - groma/systems/celld/components/isolate.md
  - groma/systems/celld/components/js-cache.md
  - groma/systems/celld/components/js-container.md
  - groma/systems/celld/components/js-websocket.md
  - groma/systems/celld/components/js.md
  - groma/systems/celld/components/kv-blob.md
  - groma/systems/celld/components/kv-cli.md
  - groma/systems/celld/components/kv-index.md
  - groma/systems/celld/components/live-app-index.md
  - groma/systems/celld/components/local-request.md
  - groma/systems/celld/components/local-store.md
  - groma/systems/celld/components/log-evict.md
  - groma/systems/celld/components/log-tier.md
  - groma/systems/celld/components/logic-cache.md
  - groma/systems/celld/components/logic-kv.md
  - groma/systems/celld/components/logic-lib.md
  - groma/systems/celld/components/logic-wake.md
  - groma/systems/celld/components/ltx-repl.md
  - groma/systems/celld/components/ltx-replication.md
  - groma/systems/celld/components/ltx-src-error.md
  - groma/systems/celld/components/ltx-src-lib.md
  - groma/systems/celld/components/ltx.md
  - groma/systems/celld/components/lz4-block.md
  - groma/systems/celld/components/machine.md
  - groma/systems/celld/components/memory.md
  - groma/systems/celld/components/message-channel.md
  - groma/systems/celld/components/mod.md
  - groma/systems/celld/components/modules.md
  - groma/systems/celld/components/node-assert.md
  - groma/systems/celld/components/node-async-hooks.md
  - groma/systems/celld/components/node-buffer.md
  - groma/systems/celld/components/node-crypto.md
  - groma/systems/celld/components/node-diagnostics-channel.md
  - groma/systems/celld/components/node-events.md
  - groma/systems/celld/components/node-log.md
  - groma/systems/celld/components/node-os.md
  - groma/systems/celld/components/node-path.md
  - groma/systems/celld/components/node-stream.md
  - groma/systems/celld/components/node-test.md
  - groma/systems/celld/components/node-timers.md
  - groma/systems/celld/components/node-util.md
  - groma/systems/celld/components/object-store.md
  - groma/systems/celld/components/observability.md
  - groma/systems/celld/components/opencode-index.md
  - groma/systems/celld/components/operator-cell.md
  - groma/systems/celld/components/otlp.md
  - groma/systems/celld/components/output-gate.md
  - groma/systems/celld/components/ownership-store.md
  - groma/systems/celld/components/paged-vfs.md
  - groma/systems/celld/components/paged.md
  - groma/systems/celld/components/peer-auth.md
  - groma/systems/celld/components/peer-probe.md
  - groma/systems/celld/components/peer.md
  - groma/systems/celld/components/persisted-capability-2node-worker.md
  - groma/systems/celld/components/persisted-capability-worker.md
  - groma/systems/celld/components/persistent-stubs-index.md
  - groma/systems/celld/components/pi-index.md
  - groma/systems/celld/components/pool.md
  - groma/systems/celld/components/pressure.md
  - groma/systems/celld/components/production.md
  - groma/systems/celld/components/profile.md
  - groma/systems/celld/components/protocol.md
  - groma/systems/celld/components/python.md
  - groma/systems/celld/components/queue-batching.md
  - groma/systems/celld/components/queue-cli.md
  - groma/systems/celld/components/queue-policy.md
  - groma/systems/celld/components/queue.md
  - groma/systems/celld/components/queues-index.md
  - groma/systems/celld/components/r2-cli.md
  - groma/systems/celld/components/r2-index.md
  - groma/systems/celld/components/r2-ops.md
  - groma/systems/celld/components/r2-store.md
  - groma/systems/celld/components/rebalance.md
  - groma/systems/celld/components/recovery-progress.md
  - groma/systems/celld/components/replica-compactor.md
  - groma/systems/celld/components/replica-url.md
  - groma/systems/celld/components/replica.md
  - groma/systems/celld/components/replication.md
  - groma/systems/celld/components/restore.md
  - groma/systems/celld/components/retained-callback-worker.md
  - groma/systems/celld/components/router-index.md
  - groma/systems/celld/components/routing.md
  - groma/systems/celld/components/rpc-index.md
  - groma/systems/celld/components/runtime.md
  - groma/systems/celld/components/s3.md
  - groma/systems/celld/components/s3gw-src-error.md
  - groma/systems/celld/components/s3gw-src-lib.md
  - groma/systems/celld/components/sandbox-index.md
  - groma/systems/celld/components/schedule.md
  - groma/systems/celld/components/secrets-fixture-index.md
  - groma/systems/celld/components/secrets.md
  - groma/systems/celld/components/server.md
  - groma/systems/celld/components/services-kv.md
  - groma/systems/celld/components/set-immediate.md
  - groma/systems/celld/components/shims.md
  - groma/systems/celld/components/sockets.md
  - groma/systems/celld/components/sqlite.md
  - groma/systems/celld/components/src-bundle.md
  - groma/systems/celld/components/startup.md
  - groma/systems/celld/components/storage-ops.md
  - groma/systems/celld/components/storage.md
  - groma/systems/celld/components/streams.md
  - groma/systems/celld/components/sweep.md
  - groma/systems/celld/components/tcp.md
  - groma/systems/celld/components/telemetry.md
  - groma/systems/celld/components/text-encoding-streams.md
  - groma/systems/celld/components/text-encoding.md
  - groma/systems/celld/components/tokio-select.md
  - groma/systems/celld/components/types.md
  - groma/systems/celld/components/url-pattern.md
  - groma/systems/celld/components/url-search-params.md
  - groma/systems/celld/components/url.md
  - groma/systems/celld/components/utils.md
  - groma/systems/celld/components/v8-strings.md
  - groma/systems/celld/components/vectordb-index.md
  - groma/systems/celld/components/wake-entry.md
  - groma/systems/celld/components/wake-format.md
  - groma/systems/celld/components/wake-record.md
  - groma/systems/celld/components/wal.md
  - groma/systems/celld/components/wasm-src-lib.md
  - groma/systems/celld/components/webapi-index.md
  - groma/systems/celld/components/workflow-index.md
  - groma/systems/celld/components/writable-stream.md
  - groma/systems/celld/components/ws-client.md
  - groma/systems/celld/components/ws-registry.md
  - groma/systems/celld/components/wsclient-index.md
  - groma/systems/celld/components/wsecho-index.md
  - groma/systems/celld/components/zlib.md
  - groma/systems/celld/containers/celld-celld/components/celld-main.md
  - groma/systems/celld/containers/celld-celld/components/cli.md
  - groma/systems/celld/containers/celld-celld/components/main-websocket.md
  - groma/systems/celld/containers/celld-celld/components/peer-tunnel.md
  - groma/systems/celld/containers/celld-celld/container.md
  - groma/systems/celld/containers/celld-s3probe/components/s3probe.md
  - groma/systems/celld/containers/celld-s3probe/container.md
  - groma/systems/celld/containers/us-s3gw/components/src-main.md
  - groma/systems/celld/containers/us-s3gw/container.md
  - groma/systems/celld/system.md
priority: medium
type: chore
ordinal: 11000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Groma adoption installed the Backlog work source but no language scanners, so `groma view --plain` reports "No component found!" and the architecture map is empty. Discovery finds Rust, JavaScript and Python projects in this repository. This task enables the recommended scanners and verifies that the TUI/view shows containers and components, without changing product code.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Groma rust, javascript and python scanners are enabled through the groma CLI
- [x] #2 groma scan produces scanned containers and components
- [x] #3 Groma view lists the scanned containers/components for the celld system (no "No component found!")
- [x] #4 No product-code changes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Run groma scanner discover/setup --no-interactive to confirm recommended scanners.
2. Add the rust, javascript and python scanner plugins with groma plugin add.
3. Run groma scan and confirm components appear in the Groma view.
4. Verify Backlog state, scanner check, git diff and working tree; record notes and close CELLD-11.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Preflight: branch celld/backlog-groma-adoption at HEAD 3337dc8. CELLD-10, decision-1 and doc-1 already existed and were Done/accepted, so no duplicate branch-policy work was created.
`groma scanner discover` and `groma scanner setup --no-interactive` recommended rust, javascript and python.
Added @groma/scanner-rust@0.2.3, @groma/scanner-javascript@0.2.3 and @groma/scanner-python@0.2.3 with `groma plugin add`.
`groma scanner check`: all three report "package found" and "project ready".
`groma scan`: created 220 elements with 178 informational findings; generated groma/systems/ with 3 containers and 210 components under the celld system.
`groma view --plain` now shows the celld system and no longer prints "No component found!"; `groma view celld --plain --max-count 12` lists its children (213 items total).
`groma lint` exits 1 with possible-duplicate findings; these are informational scan findings, not a blocker.
`pnpm check` is not applicable: the repository has no package.json and pnpm exits 1 with ERR_PNPM_NO_PKG_MANIFEST.
`git diff --check` clean. Working tree changes are groma/config.json, groma/index.md, groma/systems/ and backlog/tasks/celld-11; the unrelated pre-existing .commandcode/taste/taste/taste.md modification was left untouched. No product code changed.
Groma reports a first scan and it was not curated; curation awaits operator approval.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Enabled Groma scanners and verified the architecture view on celld/backlog-groma-adoption at HEAD 3337dc8. No merge or push was performed.

Branch policy was already recorded by CELLD-10 (Done): decision-1 "Integration branch is celld-os/rpc" (accepted), doc-1 "Branching and upstream sync" (guide), and the AGENTS.md/README.md Ramas sections. I did not duplicate them.

Groma work: added @groma/scanner-rust@0.2.3, @groma/scanner-javascript@0.2.3 and @groma/scanner-python@0.2.3 through the CLI; `groma scanner check` reports all three ready. `groma scan` created 220 elements and generated groma/systems/ with 3 containers and 210 components under the celld system. `groma view --plain` shows the celld system with no "No component found!", and `groma view celld --plain` lists 213 children. `groma lint` reports possible duplicates as informational findings (exit 1, not blocking).

Verification: Backlog task/milestone/decision/doc listings and `groma agent-instructions` succeeded; `git diff --check` clean; git status shows only groma/ and backlog/tasks/celld-11, plus an unrelated pre-existing .commandcode/taste modification left untouched. No product code changed. `pnpm check` is not applicable because the repository has no package.json (ERR_PNPM_NO_PKG_MANIFEST).

Groma reports a first scan; it was not curated without approval. Next step: when the operator approves, integrate this branch into celld-os/rpc with `git switch celld-os/rpc && git merge --ff-only celld/backlog-groma-adoption`.
<!-- SECTION:FINAL_SUMMARY:END -->
