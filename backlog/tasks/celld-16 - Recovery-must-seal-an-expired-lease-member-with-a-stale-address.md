---
id: CELLD-16
title: Recovery must seal an expired-lease member with a stale address
status: To Do
assignee: []
created_date: '2026-11-11T18:09:00Z'
labels: []
dependencies: []
references:
  - 1us-infra INFRA-9
priority: high
type: bug
ordinal: 10000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Production shape not covered by 9fdad84/f0ef747. A session's ensemble names node_9079c34a2048449be09613ef4e4c3c80 whose lease record still exists but is expired (lease=expired), not absent. Recovery tries /peer/log/seal at the old addr and counts the member inconclusive, so the session stays recovering and logs 'no complete true witness ... 1 member(s) undecided; refusing to seal while member state remains unverified'. The fix covers lease Ok(None) via absent_member_is_gone; it never runs for Ok(Some(expired)). Evidence from 1us-infra INFRA-9: after deploying f0ef747 and two celld-os restarts, session node_4cf5cb90b07b8e33ce0c205ee593201a/5d76d65eb201d3c2b120007d09063cb456e3a5710a342f7a10fda40ef01cb0aa remains state=recovering with may_have_fleet_acks=true; celld log status shows member node_9079c34a2048449be09613ef4e4c3c80 lease=expired. New churn sessions sealed, so the absent-record path works. No manual deletion of log/<session>/ was done.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A session whose ensemble member has an expired lease record and whose stored addr no longer serves that member seals after the grace, writing log/<session>.loss.json when fleet acks may exist
- [ ] #2 No manual deletion of log/<session>/ or nodes/<member>.json is required
- [ ] #3 A live or reachable member is never classified as conclusively gone (no false loss)
- [ ] #4 A regression test reproduces the expired-lease plus answered-error shape
<!-- AC:END -->
