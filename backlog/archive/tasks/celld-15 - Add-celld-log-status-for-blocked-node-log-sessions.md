---
id: CELLD-15
title: Add celld log status for blocked node-log sessions
status: Done
assignee: []
created_date: '2026-11-11T13:17:00Z'
updated_date: '2026-11-11T13:28:00Z'
labels: []
dependencies: []
references:
  - node-log
  - log-tier
  - recovery-progress
modified_files:
  - crates/celld/log_cli.rs
  - crates/celld/node_log.rs
  - crates/celld/lib.rs
  - crates/celld/main.rs
  - crates/celld/main/cli.rs
  - crates/celld/tests/log_status.rs
  - docs/guarantees.md
  - >-
    backlog/tasks/celld-15 -
    Add-celld-log-status-for-blocked-node-log-sessions.md
priority: medium
type: feature
ordinal: 10000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Recovery can refuse to seal while an ensemble member's state is undecided, and today that is only visible in node logs. Add a read-only `celld log status` that scans the bucket without a running node and reports every open or recovering session with each member's lease state (live, expired, absent, unreadable) and, for absent members, the recorded first-sighting age and grace window. No writes and no destructive prune; the automatic recovery path stays the only seal path.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 celld log status lists open and recovering node-log sessions from the bucket alone
- [x] #2 Each ensemble member is reported as live, expired, absent or unreadable
- [x] #3 An absent member shows its recorded absence age and the grace window
- [x] #4 --json emits one object per member; text output is human readable
- [x] #5 Tests cover the live, expired and absent classifications
- [x] #6 docs/guarantees.md mentions the command next to the refusal
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add a read-only `celld log status` subcommand that opens the bucket and walks `nodes/`.
2. Report each open/recovering session and each ensemble member's lease: live, expired, absent (with the recorded first-sighting age) or unreadable.
3. Reuse the recovery grace (3x TTL, min 20 s) for the estimate; do not write anything and do not add a prune path.
4. Unit-test the live/expired/absent classification and exercise the CLI against a real us-s3gw bucket.
5. Document the command next to the refusal and close CELLD-15 with evidence.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
`celld log status --bucket ... [--json]` reads the bucket directly: it walks `nodes/`, keeps records whose folded log is open or recovering, and reports each ensemble member. Live and expired come from the member lease expiry. Absent comes from a missing member record plus the durable `log/<session>/absent/e<epoch>/<member>.json` marker, so the row shows how long recovery has known the absence and how much of the 3x-TTL grace (minimum 20 s, from CELLD_TTL_MS) remains. Unreadable is a lease read error. A sealed record is skipped. The command writes nothing and deliberately has no `prune`: the automatic recovery path stays the only seal path.

Validated against a crafted us-s3gw bucket: one open session with a live member, an expired member and an absent member (marker 5 s old) printed `live`, `expired`, `absent` and skipped a sealed session; both text and `--json` matched, including the fleet-ack marker and `node-a/gen` session id.

Verification: 5 unit tests for the classification; `cargo test --test log_status` (1 passed, 1.0 s); `cargo test --workspace` green; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo fmt --check`.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `celld log status`, a read-only operator view of the node-log sessions a fleet bucket holds. It needs no running node: it lists every open or recovering session and each ensemble member as live, expired, absent (with the recorded absence age and remaining grace) or unreadable, in text or `--json`.

The command reads the same `nodes/` records and absence markers recovery uses. It writes nothing and has no `prune`; the automatic recovery path remains the only seal path, so an operator cannot skip the bounded-loss record or the bundle drain.

Wiring: new `crates/celld/log_cli.rs`, `log` action in `main/cli.rs`, dispatch in `main.rs`, module in `lib.rs`, and two read helpers in `node_log.rs`. `docs/guarantees.md` documents it next to the refusal.

Verification: 5 unit tests for live/expired/absent classification; an s3gw integration test that builds a bucket by hand and checks text, JSON and sealed-session filtering; full `cargo test --workspace` green; clippy and rustfmt clean.
<!-- SECTION:FINAL_SUMMARY:END -->
