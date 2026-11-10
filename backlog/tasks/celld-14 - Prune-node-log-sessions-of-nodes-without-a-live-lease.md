---
id: CELLD-14
title: Prune node-log sessions of nodes without a live lease
status: To Do
assignee: []
created_date: '2026-11-10T20:59:00Z'
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
<!-- AC:END -->
