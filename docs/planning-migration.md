# Backlog migration — inventory (2026-11-08)

Backlog.md is now the committed work tracker (`backlog/`, task prefix `CELLD`). This page
records which work-tracking artifacts existed, where each open item went, and what
deliberately stayed where it was.

## 1. Repository-local board

There was none. `celld` had no `docs/planning/`, `docs/TASKS.md`, `plans/`, or other
repository-local board on any branch, so nothing had to be archived. The adoption milestone
and its tasks (`CELLD-1`..`CELLD-5`) were created directly in Backlog.

## 2. Open items recorded in `docs/PR-READY.md`

`docs/PR-READY.md` is the branch summary for `celld-os/rpc`. Its "Known limits" section is
the only list of open, uncommitted work items in the repository. Those items were migrated:

| Item | Backlog |
| --- | --- |
| In-flight RPC/stream at the instant of a hard kill is ambiguous and not replayed | `CELLD-6` |
| Disposing a foreign capability does not send a remote release | `CELLD-7` |
| Operator `/metrics` has no external auth/scrape policy | `CELLD-8` |
| Nested deployed-script facets limited to the gatekeeper/gadget parent-chain shape | `CELLD-9` |

`docs/PR-READY.md` itself stays as written: it is the branch's evidence and rollout record,
not a task list.

## 3. Design and operational records — deliberately not tasks

These describe decisions, behavior or limits; they are not work items and were not copied:

- `docs/RPC-CROSS-ISOLATE.md` — design record for cross-isolate RPC. Its "Phase 2 / HTTP
  gateway" is explicitly out of scope and is not a committed task.
- `docs/guarantees.md`, `docs/cloudflare-compat.md`, `docs/limitations.md` — documented
  behavior and operational limits. A limit becomes work only when a task says so.
- `docs/rolling-deploy.md`, `docs/SECRETS.md`, `docs/security.md`, `docs/testing.md`,
  `docs/telemetry.md`, `docs/wasm.md`, `docs/RUNTIME.md` — operational and reference docs.
- `docs/FORK.md`, `docs/UPSTREAM-SYNC.md` — repository history and sync guidance.

A task may link a design record with `--ref`; the record itself is not a task.

## 4. Rules from here on

1. New work starts as a Backlog task (`backlog task create ...`), not in a doc.
2. Design records and operational docs stay in `docs/`; link them from the task.
3. Never edit `backlog/` or `groma/` files by hand: use their CLIs.
