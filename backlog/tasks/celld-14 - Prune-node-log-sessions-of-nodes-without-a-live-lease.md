---
id: CELLD-14
title: Prune node-log sessions of nodes without a live lease
status: Done
assignee: []
created_date: '2026-11-10T20:59:00Z'
updated_date: '2026-11-11T12:44:00Z'
labels: []
dependencies: []
references:
  - node-log
  - log-tier
  - recovery-progress
  - dead-node-gc
  - celld-celld
modified_files:
  - crates/celld/node_log.rs
  - crates/logic/log_tier.rs
  - crates/celld/tests/node_log_churn_takeover.rs
  - docs/guarantees.md
  - >-
    backlog/tasks/celld-14 -
    Prune-node-log-sessions-of-nodes-without-a-live-lease.md
priority: high
type: task
ordinal: 14000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A node-log session whose ensemble names a node that no longer exists can block every takeover of a cell: recovery refuses to seal because one member stays undecided (see the celld-os runbook docs/fleet-recovery.md and OS-26/OS-27 there). Today the operator deletes s3://<bucket>/log/<dead-session>/ by hand, with every node stopped. Runtime side: prune sessions whose members have no live lease (or refuse to include them in an ensemble), or expose a celld command for it, and document the refusal so operators recognise it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Sessions of nodes without a live lease are pruned automatically or by a celld command
- [x] #2 The refusal and its remedy are documented in the celld docs
- [x] #3 A churn scenario (deploy/restart twice, then a cell takeover) seals without deleting any log prefix by hand
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Reproduce the churn refusal with two nodes and a follower whose lease record is deleted.
2. Treat a member with no lease record as conclusive only after a durable absence grace.
3. Add a churn integration test that captures the refusal and the seal.
4. Document the refusal and the automatic remedy in docs/guarantees.md.
5. Verify the kernel test, clippy and the full suite; close CELLD-14.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Handoff contract (from the celld-os supervisor): (1) implement pruning of sessions whose ensemble members have no live lease, or a celld subcommand that does it, and document the refusal in the celld docs; (2) do not change the on-disk log format incompatibly; (3) reproduce first: two nodes, churn the node ids (restart), then request a takeover and show the 'no complete true witness ... undecided' refusal; (4) after the fix the same scenario seals; (5) report the branch, the commit rev and the exact reproduction, because the operator pins the rev in 1us-infra, restarts the fleet and re-runs the probes (frame, smoke, E2E) before it is considered done. The operator-side workaround today is deletion of log/<dead-session>/ with both nodes stopped (see celld-os docs/fleet-recovery.md).

Reproduction (before the fix): two nodes on us-s3gw with the fleet-fixture Worker. node-a (fleet posture) owns the cell and records a fleet-acknowledged write (may_have_fleet_acks=true). node-b joins as a bucket-posture follower, so its lease carries log: None. Kill node-b: node-a's dead-node GC deletes nodes/node-b.json instead of tombstoning it. Kill node-a and start node-c. Every takeover repeats: node-log recovery for <session>: no complete true witness among {"node-b"} and 1 member(s) undecided; refusing to seal while member state remains unverified. No log prefix was deleted by hand.

Root cause: recover_serial reads the absent member as Ok(None). lease_long_dead is only true for an existing expired record, so an absent record keeps incrementing inconclusive forever and the seal never runs.

Fix: record the first sighting at log/<session>/absent/e<epoch>/<member>.json and judge it with log_tier::absent_member_is_gone, which makes the absence conclusive only after the same 3x-TTL grace (at least 20 s). A lease read error stays undecided. Markers are cleared when the member is present again and retired with the sealed session. No log record or bundle format changed.

After: the same scenario logs the refusal on the first pass, then after the grace declares bounded loss (log/<session>.loss.json), drains the retained bundle into the per-cell prefix, seals the session, and /hello serves through node-c. The whole scenario is crates/celld/tests/node_log_churn_takeover.rs, which asserts the refusal, the loss record, the seal and the takeover.

Verification: cargo test -p celld-logic log_tier::tests (2 passed); cargo test --test node_log_churn_takeover (1 passed, 30.8 s); cargo test --workspace (all green: 51 unit plus every integration test); cargo clippy --workspace --all-targets -- -D warnings; rustfmt --check on the changed files.

The committed churn test drives the reserved __KvNamespace cell through `celld kv` (the reported shape), not a generic Durable Object: node-a creates and owns the namespace, a second put is fleet-acknowledged by node-b, node-b retires, node-a dies, and `celld kv get` on node-c exercises the refusal, the bounded-loss record, the seal and a successful read.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Fixed CELLD-14 on fix/celld-14-prune-dead-sessions: a node-log session whose ensemble names a node with no lease record now seals on its own instead of refusing every takeover forever.

Reproduction before: node-a (fleet) owns the reserved __KvNamespace cell and serves a fleet-acked write; node-b is a bucket-posture follower whose lease carries log: None. After node-b dies the dead-node GC deletes its record; after node-a dies, `celld kv get` on a new node repeated "no complete true witness among {\"node-b\"} and 1 member(s) undecided; refusing to seal while member state remains unverified".

Change: recover_serial now treats an absent member record as a dated fact. The first sighting is stored at log/<session>/absent/e<epoch>/<member>.json and log_tier::absent_member_is_gone makes it conclusive only after the existing 3x-TTL (minimum 20 s) grace. Read errors stay undecided. The marker is cleared when the member reappears and retired with the sealed session. The log record and bundle formats are unchanged.

After: the same churn logs the refusal on the first pass, then after the grace writes log/<session>.loss.json, drains the retained bundle, seals, and the KV read serves again. crates/celld/tests/node_log_churn_takeover.rs captures both halves on __KvNamespace; docs/guarantees.md documents the refusal, the automatic remedy, and why deleting log/<session>/ by hand is unsafe.

Verification: kernel tests 2/2, KV churn test 1/1 in 32.5 s, full cargo test --workspace green, clippy -D warnings clean, rustfmt clean on the changed files.
<!-- SECTION:FINAL_SUMMARY:END -->
