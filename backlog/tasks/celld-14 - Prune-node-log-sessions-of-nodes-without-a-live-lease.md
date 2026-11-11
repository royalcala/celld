---
id: CELLD-14
title: Prune node-log sessions of nodes without a live lease
status: To Do
assignee: []
created_date: '2026-11-10T20:59:00Z'
updated_date: '2026-11-11T00:17:00Z'
labels: []
dependencies: []
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
- [ ] #1 Sessions of nodes without a live lease are pruned automatically or by a celld command
- [ ] #2 The refusal and its remedy are documented in the celld docs
- [ ] #3 A churn scenario (deploy/restart twice, then a cell takeover) seals without deleting any log prefix by hand
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Handoff contract (from the celld-os supervisor): (1) implement pruning of sessions whose ensemble members have no live lease, or a celld subcommand that does it, and document the refusal in the celld docs; (2) do not change the on-disk log format incompatibly; (3) reproduce first: two nodes, churn the node ids (restart), then request a takeover and show the 'no complete true witness ... undecided' refusal; (4) after the fix the same scenario seals; (5) report the branch, the commit rev and the exact reproduction, because the operator pins the rev in 1us-infra, restarts the fleet and re-runs the probes (frame, smoke, E2E) before it is considered done. The operator-side workaround today is deletion of log/<dead-session>/ with both nodes stopped (see celld-os docs/fleet-recovery.md).
<!-- SECTION:NOTES:END -->
