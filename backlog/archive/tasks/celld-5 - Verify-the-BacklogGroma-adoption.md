---
id: CELLD-5
title: Verify the Backlog+Groma adoption
status: Done
assignee: []
created_date: '2026-11-08T16:23:00Z'
updated_date: '2026-11-08T16:24:00Z'
labels: []
milestone: m-0
dependencies:
  - CELLD-2
  - CELLD-3
  - CELLD-4
priority: medium
type: chore
ordinal: 5000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Run the integration checks after the adoption: Backlog config, task and milestone commands, Groma agent instructions and plugin listing, and confirm no product code changed.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 backlog task list/view works and reports prefix CELLD
- [x] #2 groma agent-instructions and groma plugin list work
- [x] #3 The adoption diff changes documentation, backlog/ and groma/ only
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Verified Backlog config/prefix, task and milestone commands, Groma agent instructions and work source; the diff is limited to docs, backlog/ and groma/.
<!-- SECTION:FINAL_SUMMARY:END -->
