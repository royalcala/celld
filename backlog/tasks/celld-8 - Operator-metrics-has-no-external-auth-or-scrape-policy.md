---
id: CELLD-8
title: Operator /metrics has no external auth or scrape policy
status: To Do
assignee: []
created_date: '2026-11-08T16:23:00Z'
updated_date: '2026-11-08T16:24:00Z'
labels: []
dependencies: []
references:
  - docs/PR-READY.md
  - docs/security.md
priority: medium
type: task
ordinal: 8000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The operator listener exposes /metrics with no authentication or scrape policy. Decide the exposure (bind scope, mTLS or proxy auth, scrape allow-list) and document it in docs/security.md. Migrated from docs/PR-READY.md (Known limits).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 /metrics exposure is authenticated or explicitly restricted, and docs/security.md records the policy
<!-- AC:END -->
