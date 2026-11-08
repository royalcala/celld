---
id: CELLD-7
title: Disposing a foreign capability does not send a remote release
status: To Do
assignee: []
created_date: '2026-11-08T16:23:00Z'
updated_date: '2026-11-08T16:24:00Z'
labels: []
dependencies: []
references:
  - docs/PR-READY.md
  - docs/RPC-CROSS-ISOLATE.md
priority: low
type: task
ordinal: 7000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Disposing a capability held by another isolate does not send a remote release; the owner entry is reclaimed only with its cell or isolate. Decide whether to send an explicit remote release or to document the lazy reclamation as the contract. Migrated from docs/PR-READY.md (Known limits).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Remote release behavior is implemented or documented as the intended contract
<!-- AC:END -->
