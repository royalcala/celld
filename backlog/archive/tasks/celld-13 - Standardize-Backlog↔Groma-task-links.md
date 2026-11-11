---
id: CELLD-13
title: Standardize Backlog↔Groma task links
status: Done
assignee: []
created_date: '2026-11-10T19:20:00Z'
updated_date: '2026-11-10T19:22:00Z'
labels: []
milestone: m-0
dependencies: []
references:
  - celld
  - celld-celld
  - capability
  - observability
  - facet-streams
modified_files:
  - AGENTS.md
  - backlog/tasks/celld-2 - Adopt-Groma-for-architecture-documentation.md
  - >-
    backlog/tasks/celld-6 -
    In-flight-RPC-and-streams-at-hard-kill-are-not-replayed.md
  - >-
    backlog/tasks/celld-7 -
    Disposing-a-foreign-capability-does-not-send-a-remote-release.md
  - >-
    backlog/tasks/celld-8 -
    Operator-metrics-has-no-external-auth-or-scrape-policy.md
  - >-
    backlog/tasks/celld-9 -
    Nested-deployed-script-facets-are-limited-to-the-gatekeeper-gadget-parent-chain.md
  - >-
    backlog/tasks/celld-11 -
    Enable-Groma-scanners-and-verify-architecture-view.md
  - backlog/tasks/celld-12 - Curate-the-first-Groma-architecture-scan.md
  - backlog/tasks/celld-13 - Standardize-Backlog↔Groma-task-links.md
priority: medium
type: chore
ordinal: 13000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Backlog↔Groma audit found that the curation task has no architecture references and records a directory instead of the groma paths it changed. The active tasks CELLD-6..9 have documentation-only references, and AGENTS.md does not state the linking conventions. Standardize the links so planned work attaches to real Groma elements without changing product code or the architecture map.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Adoption/curation tasks reference the celld system, containers and representative components, and list every groma/ path changed by their commits
- [x] #2 Active architecture tasks CELLD-6..9 each have at least one element or exact-file architecture ref in addition to their documentation refs
- [x] #3 AGENTS.md documents --add-ref, --doc/--ref as context only, and --modified-file after each change
- [x] #4 No product code or Groma architecture changes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Complete the audit of adoption/curation and active tasks.
2. Backfill element refs and every groma/ path from the adoption, scanner and curation commits.
3. Add refined element refs to CELLD-6..9 while keeping their documentation refs.
4. Document the three linking conventions in AGENTS.md.
5. Verify Backlog/Groma/git state, close CELLD-13 and commit only backlog/ and AGENTS.md.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Standardized: yes. Audited on branch chore/backlog-groma-links (from celld-os/rpc) and changed only backlog/ and AGENTS.md. No product code or Groma architecture was modified.

Audit before the fix:
- CELLD-2 (adoption) had no architecture refs; its groma paths from 1dde77b were already recorded.
- CELLD-11 (scanners) had no architecture refs and recorded groma/systems/ as a directory instead of the 222 paths from c41b050.
- CELLD-12 (curation) had no architecture refs and recorded only groma/project.md and groma/systems/ for the 232 paths from 70075f0.
- CELLD-6..9 had documentation-only references.
- AGENTS.md did not document the linking conventions.
- No actors or external systems are declared in Groma, so none were invented.

Changes:
- CELLD-2 refs: celld, celld-celld.
- CELLD-11 refs: celld, celld-celld, celld-s3probe, us-s3gw; modified_files now lists all 223 paths (222 from c41b050 plus the preserved groma/systems/ entry).
- CELLD-12 refs: celld, all three containers, and actor, bucket, alarm, capability, js, deploy, fleet, cli; modified_files now lists all 233 paths (232 from 70075f0 plus the preserved groma/systems/ entry).
- CELLD-6 refs: capability, peer-tunnel, http-streams (docs refs kept).
- CELLD-7 refs: capability, cell-dispatch (docs refs kept).
- CELLD-8 refs: observability, telemetry (docs refs kept).
- CELLD-9 refs: cell-dispatch, facet-streams, harness (docs refs kept).
- AGENTS.md now documents --add-ref <element-id|file>, --doc/--ref as context only, and --modified-file after each change for curation tasks.

Verification: groma view --plain shows the celld system; backlog task view CELLD-13 --plain shows the refs and modified files; git diff --check is clean; git status shows only backlog/ and AGENTS.md plus the unrelated pre-existing .commandcode/taste modification. No groma/ or product-code changes.
<!-- SECTION:FINAL_SUMMARY:END -->
