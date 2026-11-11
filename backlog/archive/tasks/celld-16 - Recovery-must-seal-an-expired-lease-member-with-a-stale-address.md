---
id: CELLD-16
title: Recovery must seal an expired-lease member with a stale address
status: Done
assignee: []
created_date: '2026-11-11T18:09:00Z'
updated_date: '2026-11-11T19:05:00Z'
labels: []
dependencies: []
references:
  - 1us-infra INFRA-9
  - node-log
  - log-tier
  - recovery-progress
modified_files:
  - crates/celld/node_log.rs
  - crates/celld/tests/node_log_expired_lease.rs
  - docs/guarantees.md
  - >-
    backlog/tasks/celld-16 -
    Recovery-must-seal-an-expired-lease-member-with-a-stale-address.md
priority: high
type: bug
ordinal: 10000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Production shape not covered by 9fdad84/f0ef747. A session's ensemble names node_9079c34a2048449be09613ef4e4c3c80 whose lease record still exists but is expired (lease=expired), not absent. Recovery tries /peer/log/seal at the old addr and counts the member inconclusive, so the session stays recovering and logs 'no complete true witness ... 1 member(s) undecided; refusing to seal while member state remains unverified'. The fix covers lease Ok(None) via absent_member_is_gone; it never runs for Ok(Some(expired)). Evidence from 1us-infra INFRA-9: after deploying f0ef747 and two celld-os restarts, session node_4cf5cb90b07b8e33ce0c205ee593201a/5d76d65eb201d3c2b120007d09063cb456e3a5710a342f7a10fda40ef01cb0aa remains state=recovering with may_have_fleet_acks=true; celld log status shows member node_9079c34a2048449be09613ef4e4c3c80 lease=expired. New churn sessions sealed, so the absent-record path works. No manual deletion of log/<session>/ was done.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A session whose ensemble member has an expired lease record and whose stored addr no longer serves that member seals after the grace, writing log/<session>.loss.json when fleet acks may exist
- [x] #2 No manual deletion of log/<session>/ or nodes/<member>.json is required
- [x] #3 A live or reachable member is never classified as conclusively gone (no false loss)
- [x] #4 A regression test reproduces the expired-lease plus answered-error shape
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Reproduce the production shape in a regression test: an ensemble member whose lease record is expired past the grace and whose stored addr answers 404.
2. Change the seal-error classification so an answered error from a long-dead lease is conclusively gone, while a live lease or one inside the grace stays undecided.
3. Keep a reachable member with a valid seal/tail witness or inconsistent retained range undecided; do not weaken AC3.
4. Document the stale-address case next to the refusal and the automatic loss record.
5. Run the full suite, commit, merge/push, then ask 1us-infra to bump the pin and re-run AC6.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Reproduction before: the new regression test replaces the follower's lease with a long-expired record whose stored addr is a live 404 server, then takes the namespace over on node-c. With the old `answered || lease_live || !lease_long_dead` condition it fails after 67 s with the production refusal: `no complete true witness among {"node-b"} and 1 member(s) undecided; refusing to seal while member state remains unverified`.

After: the same test passes in 8 s. The stale address is probed, recovery declares `log/<session>.loss.json`, drains the bundle, seals, and `kv get` serves. The condition now increments `inconclusive` only while the lease is live or inside the grace. A valid seal/tail from a reachable member still goes through the witness and retained-range checks, so a reachable member with rows is not classified gone. The existing absent-record churn test still passes (31.9 s).

Verification: `cargo test --test node_log_expired_lease` (1 passed, 8.0 s), `cargo test --test node_log_churn_takeover` (1 passed, 31.9 s), `cargo test --workspace` green, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`. No log format change and no manual deletion.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Fixed CELLD-16: recovery now seals a session whose ensemble member holds an expired lease record and no longer serves at the stored address.

An answered HTTP or protocol error used to count as undecided even after the lease was long dead, so a stale address that answered 404 kept the legacy session recovering forever. The seal-error path now keeps the member undecided only while its lease is live or inside the 3x-TTL grace; past the grace the stale address is not a witness, recovery declares the bounded loss and seals. A valid seal/tail from a reachable member still goes through the existing witness and retained-range checks.

Added `crates/celld/tests/node_log_expired_lease.rs`, which reproduces the production shape with a fake 404 peer. It fails on the old condition with the exact refusal and passes after the fix, with the loss record and seal. The existing absent-record churn test still passes.

Verification: new test 1/1 (8.0 s), churn test 1/1 (31.9 s), full workspace green, clippy and rustfmt clean.
<!-- SECTION:FINAL_SUMMARY:END -->
