---
id: CELLD-1
title: Adopt Backlog.md as the committed work tracker
status: Done
assignee: []
created_date: '2026-11-08T16:23:00Z'
updated_date: '2026-11-08T16:24:00Z'
labels: []
milestone: m-0
dependencies: []
modified_files:
  - backlog/config.yml
  - backlog/milestones/m-0 - backlog+groma-adoption.md
priority: high
type: chore
ordinal: 1000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Initialize Backlog.md for celld with task prefix CELLD, no automatic agent-instruction writes, and a milestone for this adoption. Backlog becomes the committed tracker for planned work; design records and operational docs stay in docs/.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 backlog/config.yml exists with task_prefix: CELLD
- [x] #2 Milestone 'Backlog+Groma adoption' exists
- [x] #3 backlog task list --plain works from the repo root
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Backlog.md initialized under backlog/ with task_prefix CELLD and the adoption milestone m-0. Verified with backlog task list --plain.
<!-- SECTION:FINAL_SUMMARY:END -->
