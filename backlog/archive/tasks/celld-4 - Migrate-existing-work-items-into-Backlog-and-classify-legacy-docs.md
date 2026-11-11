---
id: CELLD-4
title: Migrate existing work items into Backlog and classify legacy docs
status: Done
assignee: []
created_date: '2026-11-08T16:23:00Z'
updated_date: '2026-11-08T16:24:00Z'
labels: []
milestone: m-0
dependencies:
  - CELLD-1
modified_files:
  - docs/planning-migration.md
priority: high
type: chore
ordinal: 4000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Inventory the repository work-tracking artifacts (docs/PR-READY.md open items, docs/limitations.md operational limits, docs/RPC-CROSS-ISOLATE.md scope, AGENTS.md Estado de trabajo) and migrate each tracked work item into Backlog, or classify it explicitly as a design record, operational limit or historical note. Nothing is deleted.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Every open item in docs/PR-READY.md has a Backlog task or an explicit classification
- [x] #2 Design and operational docs are classified, not copied as tasks
- [x] #3 Migration inventory committed under docs/planning-migration.md
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Migrated the four open known limits from docs/PR-READY.md to CELLD-6..CELLD-9 and recorded the inventory in docs/planning-migration.md.
<!-- SECTION:FINAL_SUMMARY:END -->
