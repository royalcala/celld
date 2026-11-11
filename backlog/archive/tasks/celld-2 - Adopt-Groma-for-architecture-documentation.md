---
id: CELLD-2
title: Adopt Groma for architecture documentation
status: Done
assignee: []
created_date: '2026-11-08T16:23:00Z'
updated_date: '2026-11-10T19:21:00Z'
labels: []
milestone: m-0
dependencies:
  - CELLD-1
references:
  - celld
  - celld-celld
modified_files:
  - groma/config.json
  - groma/index.md
  - groma/project.md
  - AGENTS.md
priority: high
type: chore
ordinal: 2000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Initialize Groma at the repository root and add its Backlog work-source plugin. Capture the agent instructions so the architecture map and task links stay in step.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 groma/ exists and is committed
- [x] #2 groma plugin add backlog registered in groma/config.json
- [x] #3 groma agent-instructions runs and is referenced from AGENTS.md
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Groma initialized in groma/ with the backlog work source (1.55.3). groma init appended the standard Groma block to AGENTS.md.
<!-- SECTION:FINAL_SUMMARY:END -->
