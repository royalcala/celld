---
id: CELLD-12
title: Curate the first Groma architecture scan
status: Done
assignee: []
created_date: '2026-11-10T18:49:00Z'
updated_date: '2026-11-10T18:59:00Z'
labels: []
milestone: m-0
dependencies: []
modified_files:
  - groma/project.md
  - groma/systems/
priority: medium
type: chore
ordinal: 12000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Groma scanners produced the first architecture scan (3 containers, 210 components) but every element still lacks a description or overview. A first scan is evidence, not the finished map: a new reader cannot tell the system purpose, the container boundaries, or the main responsibilities. This task curates that scan through the groma CLI without changing product code.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Project, system and container records have descriptions and overviews
- [x] #2 Core runtime components are described and grouped by domain
- [x] #3 Test, fixture and example components are excluded or explicitly reported
- [x] #4 groma scan remains stable and groma view renders the curated map
- [x] #5 No product-code changes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Inspect the scan, container boundaries and source documentation.
2. Settle structure: move runtime components under the celld node container and test tooling under its own containers.
3. Describe the project, system, containers and components from source documentation.
4. Group components by domain and label examples, fixtures and test tooling.
5. Rescan for stability, verify the views and git state, then close the task.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Preflight: branch celld/backlog-groma-adoption at c41b050; CELLD-10 and CELLD-11 were already Done.
Structure: moved 168 runtime components under the celld-celld container (renamed "celld node") and the seven tools/s3gw components under us-s3gw. Kept the two test containers visible and described as test tooling.
Meaning: described the project, the celld system, all three containers, 172 runtime components, 30 example components, 5 test fixtures and 9 tooling components. Every C4 element now has a description; only groma/index.md intentionally has none.
Groups: Cell runtime, Storage and replication, Durable Object services, RPC and networking, JavaScript runtime, Deployment and secrets, Fleet operations, Operator CLI, plus Examples, Test fixtures and S3 gateway.
Stability: two consecutive scans report created 0, refreshed 216; the first-scan note is gone. groma view --plain shows the celld system; celld-celld lists 172 components and celld lists 38 children.
Test/example/tooling: kept in the map and labeled, not hidden. I did not hand-edit groma/config.json; excluding those paths can be a separate follow-up if the operator wants a product-only map.
Verification: backlog listings and groma agent-instructions OK; git diff --check clean; no product code changed. pnpm check is not applicable (no package.json, ERR_PNPM_NO_PKG_MANIFEST).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Curated the first Groma architecture scan on celld/backlog-groma-adoption at HEAD c41b050, using only the groma CLI for architecture. No product code changed, and no merge or push was performed.

Structure: moved 168 runtime components under the celld-celld container (now titled "celld node") and the seven tools/s3gw components under the us-s3gw container. The s3probe and s3gw containers stay visible and are described as test tooling.

Meaning: described the project, the celld system, all three containers, 172 runtime components, 30 example components, 5 test fixtures and 9 tooling components. Every C4 element now has a description; the only file without one is groma/index.md, which is an index.

Groups: Cell runtime, Storage and replication, Durable Object services, RPC and networking, JavaScript runtime, Deployment and secrets, Fleet operations, Operator CLI, plus Examples, Test fixtures and S3 gateway.

Stability and views: two consecutive scans report created 0, refreshed 216. The first-scan note is gone. groma view --plain shows the celld system; groma view celld-celld --plain lists 172 components and groma view celld --plain lists 38 children.

Verification: backlog task/milestone/decision/doc listings and groma agent-instructions succeeded; groma scanner check reports all three scanners ready; git diff --check is clean; git status shows only groma/ and the CELLD-12 task, plus an unrelated pre-existing .commandcode/taste modification left untouched. pnpm check is not applicable because the repository has no package.json (ERR_PNPM_NO_PKG_MANIFEST).

Test, fixture and example components were kept and labeled rather than hidden, because I did not hand-edit groma/config.json. A product-only map remains an optional follow-up scanner exclusion.
<!-- SECTION:FINAL_SUMMARY:END -->
