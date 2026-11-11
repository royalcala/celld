---
id: CELLD-12
title: Curate the first Groma architecture scan
status: Done
assignee: []
created_date: '2026-11-10T18:49:00Z'
updated_date: '2026-11-10T19:21:00Z'
labels: []
milestone: m-0
dependencies: []
references:
  - celld
  - celld-celld
  - celld-s3probe
  - us-s3gw
  - actor
  - bucket
  - alarm
  - capability
  - js
  - deploy
  - fleet
  - cli
modified_files:
  - groma/project.md
  - groma/systems/
  - groma/systems/celld/components/alarm-index.md
  - groma/systems/celld/components/alarm-websocket-worker.md
  - groma/systems/celld/components/async-index.md
  - groma/systems/celld/components/body-index.md
  - groma/systems/celld/components/cancellation-index.md
  - groma/systems/celld/components/cell-runtime.md
  - groma/systems/celld/components/container-index.md
  - groma/systems/celld/components/counter-index.md
  - groma/systems/celld/components/cron-index.md
  - groma/systems/celld/components/d1-index.md
  - groma/systems/celld/components/db.md
  - groma/systems/celld/components/deploy.md
  - groma/systems/celld/components/dynamic-worker-tails-index.md
  - groma/systems/celld/components/entry.md
  - groma/systems/celld/components/execute-code-tails-worker.md
  - groma/systems/celld/components/facets-index.md
  - groma/systems/celld/components/fleet-fixture-index.md
  - groma/systems/celld/components/format.md
  - groma/systems/celld/components/hello-index.md
  - groma/systems/celld/components/host-channels.md
  - groma/systems/celld/components/http-streams.md
  - groma/systems/celld/components/kv-index.md
  - groma/systems/celld/components/ltx.md
  - groma/systems/celld/components/opencode-index.md
  - groma/systems/celld/components/persisted-capability-2node-worker.md
  - groma/systems/celld/components/persisted-capability-worker.md
  - groma/systems/celld/components/persistent-stubs-index.md
  - groma/systems/celld/components/pi-index.md
  - groma/systems/celld/components/pressure.md
  - groma/systems/celld/components/queues-index.md
  - groma/systems/celld/components/r2-index.md
  - groma/systems/celld/components/retained-callback-worker.md
  - groma/systems/celld/components/router-index.md
  - groma/systems/celld/components/rpc-index.md
  - groma/systems/celld/components/sandbox-index.md
  - groma/systems/celld/components/secrets-fixture-index.md
  - groma/systems/celld/components/server.md
  - groma/systems/celld/components/startup.md
  - groma/systems/celld/components/storage-ops.md
  - groma/systems/celld/components/vectordb-index.md
  - groma/systems/celld/components/wasm-src-lib.md
  - groma/systems/celld/components/webapi-index.md
  - groma/systems/celld/components/workflow-index.md
  - groma/systems/celld/components/ws-registry.md
  - groma/systems/celld/components/wsclient-index.md
  - groma/systems/celld/components/wsecho-index.md
  - groma/systems/celld/containers/celld-celld/components/actor.md
  - groma/systems/celld/containers/celld-celld/components/alarm.md
  - groma/systems/celld/containers/celld-celld/components/assets.md
  - groma/systems/celld/containers/celld-celld/components/asyncrt.md
  - groma/systems/celld/containers/celld-celld/components/atob-btoa.md
  - groma/systems/celld/containers/celld-celld/components/bootstrap.md
  - groma/systems/celld/containers/celld-celld/components/bucket.md
  - groma/systems/celld/containers/celld-celld/components/byte-streams.md
  - groma/systems/celld/containers/celld-celld/components/capability.md
  - groma/systems/celld/containers/celld-celld/components/cell-cli.md
  - groma/systems/celld/containers/celld-celld/components/cell-dispatch.md
  - groma/systems/celld/containers/celld-celld/components/cell-host.md
  - groma/systems/celld/containers/celld-celld/components/cell-runtime.md
  - groma/systems/celld/containers/celld-celld/components/cell.md
  - groma/systems/celld/containers/celld-celld/components/celld-container.md
  - >-
    groma/systems/celld/containers/celld-celld/components/celld-crates-celld-js-crypto-887dcaf0.md
  - >-
    groma/systems/celld/containers/celld-celld/components/celld-crates-celld-js-crypto-8d3fd54d.md
  - >-
    groma/systems/celld/containers/celld-celld/components/celld-crates-celld-js-html-rewriter-6b05c950.md
  - >-
    groma/systems/celld/containers/celld-celld/components/celld-crates-celld-js-html-rewriter-e3c2c7ec.md
  - >-
    groma/systems/celld/containers/celld-celld/components/celld-crates-celld-python-worker-668a5ea2.md
  - >-
    groma/systems/celld/containers/celld-celld/components/celld-crates-celld-python-worker-d03f3928.md
  - groma/systems/celld/containers/celld-celld/components/celld-lib.md
  - groma/systems/celld/containers/celld-celld/components/celld-main.md
  - groma/systems/celld/containers/celld-celld/components/celld-wake.md
  - groma/systems/celld/containers/celld-celld/components/clean-reload.md
  - groma/systems/celld/containers/celld-celld/components/cli-options.md
  - groma/systems/celld/containers/celld-celld/components/cli-output.md
  - groma/systems/celld/containers/celld-celld/components/cli.md
  - groma/systems/celld/containers/celld-celld/components/client-bundle.md
  - groma/systems/celld/containers/celld-celld/components/codec.md
  - groma/systems/celld/containers/celld-celld/components/compaction-level.md
  - groma/systems/celld/containers/celld-celld/components/compactor.md
  - groma/systems/celld/containers/celld-celld/components/compression-streams.md
  - groma/systems/celld/containers/celld-celld/components/control-plane.md
  - groma/systems/celld/containers/celld-celld/components/cron.md
  - groma/systems/celld/containers/celld-celld/components/d1-cli.md
  - groma/systems/celld/containers/celld-celld/components/db.md
  - groma/systems/celld/containers/celld-celld/components/dead-node-gc.md
  - >-
    groma/systems/celld/containers/celld-celld/components/dead-node-reconciliation.md
  - groma/systems/celld/containers/celld-celld/components/deploy.md
  - groma/systems/celld/containers/celld-celld/components/dev.md
  - groma/systems/celld/containers/celld-celld/components/docker.md
  - groma/systems/celld/containers/celld-celld/components/drain-token.md
  - groma/systems/celld/containers/celld-celld/components/drain.md
  - groma/systems/celld/containers/celld-celld/components/durability.md
  - groma/systems/celld/containers/celld-celld/components/effects.md
  - groma/systems/celld/containers/celld-celld/components/engine-api.md
  - groma/systems/celld/containers/celld-celld/components/env-vars.md
  - groma/systems/celld/containers/celld-celld/components/epoch-gc.md
  - groma/systems/celld/containers/celld-celld/components/epochs.md
  - groma/systems/celld/containers/celld-celld/components/event-source.md
  - groma/systems/celld/containers/celld-celld/components/facet-streams.md
  - groma/systems/celld/containers/celld-celld/components/fatal.md
  - groma/systems/celld/containers/celld-celld/components/file.md
  - groma/systems/celld/containers/celld-celld/components/fleet.md
  - groma/systems/celld/containers/celld-celld/components/format.md
  - groma/systems/celld/containers/celld-celld/components/gate.md
  - groma/systems/celld/containers/celld-celld/components/generation.md
  - groma/systems/celld/containers/celld-celld/components/harness.md
  - groma/systems/celld/containers/celld-celld/components/headers.md
  - groma/systems/celld/containers/celld-celld/components/host-channels.md
  - groma/systems/celld/containers/celld-celld/components/host-services.md
  - groma/systems/celld/containers/celld-celld/components/host.md
  - groma/systems/celld/containers/celld-celld/components/http-streams.md
  - groma/systems/celld/containers/celld-celld/components/http.md
  - >-
    groma/systems/celld/containers/celld-celld/components/identity-transform-stream.md
  - >-
    groma/systems/celld/containers/celld-celld/components/input-gate-lifecycle.md
  - groma/systems/celld/containers/celld-celld/components/isolate.md
  - groma/systems/celld/containers/celld-celld/components/js-cache.md
  - groma/systems/celld/containers/celld-celld/components/js-container.md
  - groma/systems/celld/containers/celld-celld/components/js-websocket.md
  - groma/systems/celld/containers/celld-celld/components/js.md
  - groma/systems/celld/containers/celld-celld/components/kv-blob.md
  - groma/systems/celld/containers/celld-celld/components/kv-cli.md
  - groma/systems/celld/containers/celld-celld/components/local-request.md
  - groma/systems/celld/containers/celld-celld/components/local-store.md
  - groma/systems/celld/containers/celld-celld/components/log-evict.md
  - groma/systems/celld/containers/celld-celld/components/log-tier.md
  - groma/systems/celld/containers/celld-celld/components/logic-cache.md
  - groma/systems/celld/containers/celld-celld/components/logic-kv.md
  - groma/systems/celld/containers/celld-celld/components/logic-lib.md
  - groma/systems/celld/containers/celld-celld/components/logic-wake.md
  - groma/systems/celld/containers/celld-celld/components/ltx-repl.md
  - groma/systems/celld/containers/celld-celld/components/ltx-replication.md
  - groma/systems/celld/containers/celld-celld/components/ltx-src-error.md
  - groma/systems/celld/containers/celld-celld/components/ltx-src-lib.md
  - groma/systems/celld/containers/celld-celld/components/ltx.md
  - groma/systems/celld/containers/celld-celld/components/lz4-block.md
  - groma/systems/celld/containers/celld-celld/components/machine.md
  - groma/systems/celld/containers/celld-celld/components/main-websocket.md
  - groma/systems/celld/containers/celld-celld/components/memory.md
  - groma/systems/celld/containers/celld-celld/components/message-channel.md
  - groma/systems/celld/containers/celld-celld/components/mod.md
  - groma/systems/celld/containers/celld-celld/components/modules.md
  - groma/systems/celld/containers/celld-celld/components/node-assert.md
  - groma/systems/celld/containers/celld-celld/components/node-async-hooks.md
  - groma/systems/celld/containers/celld-celld/components/node-buffer.md
  - groma/systems/celld/containers/celld-celld/components/node-crypto.md
  - >-
    groma/systems/celld/containers/celld-celld/components/node-diagnostics-channel.md
  - groma/systems/celld/containers/celld-celld/components/node-events.md
  - groma/systems/celld/containers/celld-celld/components/node-log.md
  - groma/systems/celld/containers/celld-celld/components/node-os.md
  - groma/systems/celld/containers/celld-celld/components/node-path.md
  - groma/systems/celld/containers/celld-celld/components/node-stream.md
  - groma/systems/celld/containers/celld-celld/components/node-test.md
  - groma/systems/celld/containers/celld-celld/components/node-timers.md
  - groma/systems/celld/containers/celld-celld/components/node-util.md
  - groma/systems/celld/containers/celld-celld/components/object-store.md
  - groma/systems/celld/containers/celld-celld/components/observability.md
  - groma/systems/celld/containers/celld-celld/components/operator-cell.md
  - groma/systems/celld/containers/celld-celld/components/otlp.md
  - groma/systems/celld/containers/celld-celld/components/output-gate.md
  - groma/systems/celld/containers/celld-celld/components/ownership-store.md
  - groma/systems/celld/containers/celld-celld/components/paged-vfs.md
  - groma/systems/celld/containers/celld-celld/components/paged.md
  - groma/systems/celld/containers/celld-celld/components/peer-auth.md
  - groma/systems/celld/containers/celld-celld/components/peer-probe.md
  - groma/systems/celld/containers/celld-celld/components/peer-tunnel.md
  - groma/systems/celld/containers/celld-celld/components/peer.md
  - groma/systems/celld/containers/celld-celld/components/pool.md
  - groma/systems/celld/containers/celld-celld/components/pressure.md
  - groma/systems/celld/containers/celld-celld/components/production.md
  - groma/systems/celld/containers/celld-celld/components/profile.md
  - groma/systems/celld/containers/celld-celld/components/protocol.md
  - groma/systems/celld/containers/celld-celld/components/python.md
  - groma/systems/celld/containers/celld-celld/components/queue-batching.md
  - groma/systems/celld/containers/celld-celld/components/queue-cli.md
  - groma/systems/celld/containers/celld-celld/components/queue-policy.md
  - groma/systems/celld/containers/celld-celld/components/queue.md
  - groma/systems/celld/containers/celld-celld/components/r2-cli.md
  - groma/systems/celld/containers/celld-celld/components/r2-ops.md
  - groma/systems/celld/containers/celld-celld/components/r2-store.md
  - groma/systems/celld/containers/celld-celld/components/rebalance.md
  - groma/systems/celld/containers/celld-celld/components/recovery-progress.md
  - groma/systems/celld/containers/celld-celld/components/replica-compactor.md
  - groma/systems/celld/containers/celld-celld/components/replica-url.md
  - groma/systems/celld/containers/celld-celld/components/replica.md
  - groma/systems/celld/containers/celld-celld/components/replication.md
  - groma/systems/celld/containers/celld-celld/components/restore.md
  - groma/systems/celld/containers/celld-celld/components/routing.md
  - groma/systems/celld/containers/celld-celld/components/runtime.md
  - groma/systems/celld/containers/celld-celld/components/schedule.md
  - groma/systems/celld/containers/celld-celld/components/secrets.md
  - groma/systems/celld/containers/celld-celld/components/services-kv.md
  - groma/systems/celld/containers/celld-celld/components/set-immediate.md
  - groma/systems/celld/containers/celld-celld/components/shims.md
  - groma/systems/celld/containers/celld-celld/components/sockets.md
  - groma/systems/celld/containers/celld-celld/components/sqlite.md
  - groma/systems/celld/containers/celld-celld/components/src-bundle.md
  - groma/systems/celld/containers/celld-celld/components/startup.md
  - groma/systems/celld/containers/celld-celld/components/storage-ops.md
  - groma/systems/celld/containers/celld-celld/components/storage.md
  - groma/systems/celld/containers/celld-celld/components/streams.md
  - groma/systems/celld/containers/celld-celld/components/sweep.md
  - groma/systems/celld/containers/celld-celld/components/tcp.md
  - groma/systems/celld/containers/celld-celld/components/telemetry.md
  - >-
    groma/systems/celld/containers/celld-celld/components/text-encoding-streams.md
  - groma/systems/celld/containers/celld-celld/components/text-encoding.md
  - groma/systems/celld/containers/celld-celld/components/tokio-select.md
  - groma/systems/celld/containers/celld-celld/components/types.md
  - groma/systems/celld/containers/celld-celld/components/url-pattern.md
  - groma/systems/celld/containers/celld-celld/components/url-search-params.md
  - groma/systems/celld/containers/celld-celld/components/url.md
  - groma/systems/celld/containers/celld-celld/components/v8-strings.md
  - groma/systems/celld/containers/celld-celld/components/wake-entry.md
  - groma/systems/celld/containers/celld-celld/components/wake-format.md
  - groma/systems/celld/containers/celld-celld/components/wake-record.md
  - groma/systems/celld/containers/celld-celld/components/wal.md
  - groma/systems/celld/containers/celld-celld/components/writable-stream.md
  - groma/systems/celld/containers/celld-celld/components/ws-client.md
  - groma/systems/celld/containers/celld-celld/components/ws-registry.md
  - groma/systems/celld/containers/celld-celld/components/zlib.md
  - groma/systems/celld/containers/celld-celld/container.md
  - groma/systems/celld/containers/celld-s3probe/components/s3probe.md
  - groma/systems/celld/containers/celld-s3probe/container.md
  - groma/systems/celld/containers/us-s3gw/components/checksum.md
  - groma/systems/celld/containers/us-s3gw/components/fs.md
  - groma/systems/celld/containers/us-s3gw/components/live-app-index.md
  - groma/systems/celld/containers/us-s3gw/components/s3.md
  - groma/systems/celld/containers/us-s3gw/components/s3gw-src-error.md
  - groma/systems/celld/containers/us-s3gw/components/s3gw-src-lib.md
  - groma/systems/celld/containers/us-s3gw/components/src-main.md
  - groma/systems/celld/containers/us-s3gw/components/utils.md
  - groma/systems/celld/containers/us-s3gw/container.md
  - groma/systems/celld/system.md
priority: medium
type: chore
ordinal: 12000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Groma scanners produced the first architecture scan (3 containers, 210 components) but every element still lacks a description or overview. A first scan is evidence, not the finished map: a new reader cannot tell the system purpose, the container boundaries, or the main responsibilities. This task curates that scan through the groma CLI without changing product code.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Project, system and container records have descriptions and overviews
- [x] #2 Core runtime components are described and grouped by domain
- [x] #3 Test, fixture and example components are excluded or explicitly reported
- [x] #4 groma scan remains stable and groma view renders the curated map
- [x] #5 No product-code changes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Inspect the scan, container boundaries and source documentation.
2. Settle structure: move runtime components under the celld node container and test tooling under its own containers.
3. Describe the project, system, containers and components from source documentation.
4. Group components by domain and label examples, fixtures and test tooling.
5. Rescan for stability, verify the views and git state, then close the task.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Preflight: branch celld/backlog-groma-adoption at c41b050; CELLD-10 and CELLD-11 were already Done.
Structure: moved 168 runtime components under the celld-celld container (renamed "celld node") and the seven tools/s3gw components under us-s3gw. Kept the two test containers visible and described as test tooling.
Meaning: described the project, the celld system, all three containers, 172 runtime components, 30 example components, 5 test fixtures and 9 tooling components. Every C4 element now has a description; only groma/index.md intentionally has none.
Groups: Cell runtime, Storage and replication, Durable Object services, RPC and networking, JavaScript runtime, Deployment and secrets, Fleet operations, Operator CLI, plus Examples, Test fixtures and S3 gateway.
Stability: two consecutive scans report created 0, refreshed 216; the first-scan note is gone. groma view --plain shows the celld system; celld-celld lists 172 components and celld lists 38 children.
Test/example/tooling: kept in the map and labeled, not hidden. I did not hand-edit groma/config.json; excluding those paths can be a separate follow-up if the operator wants a product-only map.
Verification: backlog listings and groma agent-instructions OK; git diff --check clean; no product code changed. pnpm check is not applicable (no package.json, ERR_PNPM_NO_PKG_MANIFEST).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Curated the first Groma architecture scan on celld/backlog-groma-adoption at HEAD c41b050, using only the groma CLI for architecture. No product code changed, and no merge or push was performed.

Structure: moved 168 runtime components under the celld-celld container (now titled "celld node") and the seven tools/s3gw components under the us-s3gw container. The s3probe and s3gw containers stay visible and are described as test tooling.

Meaning: described the project, the celld system, all three containers, 172 runtime components, 30 example components, 5 test fixtures and 9 tooling components. Every C4 element now has a description; the only file without one is groma/index.md, which is an index.

Groups: Cell runtime, Storage and replication, Durable Object services, RPC and networking, JavaScript runtime, Deployment and secrets, Fleet operations, Operator CLI, plus Examples, Test fixtures and S3 gateway.

Stability and views: two consecutive scans report created 0, refreshed 216. The first-scan note is gone. groma view --plain shows the celld system; groma view celld-celld --plain lists 172 components and groma view celld --plain lists 38 children.

Verification: backlog task/milestone/decision/doc listings and groma agent-instructions succeeded; groma scanner check reports all three scanners ready; git diff --check is clean; git status shows only groma/ and the CELLD-12 task, plus an unrelated pre-existing .commandcode/taste modification left untouched. pnpm check is not applicable because the repository has no package.json (ERR_PNPM_NO_PKG_MANIFEST).

Test, fixture and example components were kept and labeled rather than hidden, because I did not hand-edit groma/config.json. A product-only map remains an optional follow-up scanner exclusion.
<!-- SECTION:FINAL_SUMMARY:END -->
