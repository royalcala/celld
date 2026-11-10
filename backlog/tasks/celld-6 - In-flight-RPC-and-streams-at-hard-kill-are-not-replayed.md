---
id: CELLD-6
title: In-flight RPC and streams at hard kill are not replayed
status: To Do
assignee: []
created_date: '2026-11-08T16:23:00Z'
updated_date: '2026-11-10T19:21:00Z'
labels: []
dependencies: []
references:
  - docs/PR-READY.md
  - docs/guarantees.md
  - docs/RPC-CROSS-ISOLATE.md
  - capability
  - peer-tunnel
  - http-streams
priority: low
type: task
ordinal: 6000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
An in-flight RPC or stream at the instant of a hard node kill is ambiguous: the runtime does not replay it. Decide whether the client re-acquires or the runtime offers a bounded replay/repair path, and document the chosen behavior. Migrated from docs/PR-READY.md (Known limits).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The hard-kill in-flight behavior is decided and documented, with a test if the runtime changes
<!-- AC:END -->
