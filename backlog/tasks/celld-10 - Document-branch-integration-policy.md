---
id: CELLD-10
title: Document branch integration policy
status: Done
assignee: []
created_date: '2026-11-10T17:06:00Z'
updated_date: '2026-11-10T17:07:00Z'
labels: []
milestone: m-0
dependencies: []
modified_files:
  - AGENTS.md
  - README.md
  - backlog/decisions/decision-1 - Integration-branch-is-celld-os-rpc.md
  - backlog/docs/doc-1 - Branching-and-upstream-sync.md
priority: medium
type: docs
ordinal: 10000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
celld now tracks work in Backlog and architecture in Groma, but the repository does not say which branch is the integration line. Without a written policy, the 1us/celld-os adaptation in celld-os/rpc could be merged into upstream/main or origin/main, and new work could start from the wrong base. This task records that policy in AGENTS.md and README.md and keeps Backlog as the durable record of the decision.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 AGENTS.md has a Ramas section with upstream/main, origin/main, celld-os/rpc and the adoption branch
- [x] #2 README.md links the branch policy
- [x] #3 Backlog decision 'Integration branch is celld-os/rpc' exists as accepted
- [x] #4 A Backlog doc describes integration and upstream sync
- [x] #5 No merge, push, or product-code changes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Record the accepted Backlog decision naming celld-os/rpc as the integration branch.
2. Create the Backlog guide doc with the branch map, integration and upstream-sync steps.
3. Add the Ramas section to AGENTS.md linking decision and doc.
4. Add the short Ramas note to README.md linking AGENTS.md and the decision.
5. Verify Backlog/Groma output, pnpm check and git diff; close CELLD-10.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Preflight audit: branch celld/backlog-groma-adoption, HEAD 1dde77b, clean tree, and `git merge-base --is-ancestor celld-os/rpc HEAD` succeeded.
No agent name was exposed by the environment or Herdr, so the task was left unassigned instead of inventing one.
Created decision-1 (accepted) and doc-1 (guide) with the CLI; doc body written via `backlog doc update`.
Verification: `backlog task list --plain`, `backlog milestone list`, `backlog decision list`, `backlog doc list` and `groma agent-instructions` all return expected output. `git diff --check` clean. `git status --short` shows only AGENTS.md, README.md and backlog/ (no product code, no groma/).
`pnpm check` is not applicable: the repo has no package.json and pnpm exits 1 with ERR_PNPM_NO_PKG_MANIFEST. No Rust code changed, so no product build/test was required.
No merge, no push, and no local main was created.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Documented celld's branch integration policy on celld/backlog-groma-adoption at commit 1dde77b (base celld-os/rpc @ acc1a06). No merge or push was performed.

Created Backlog records: decision-1 "Integration branch is celld-os/rpc" (accepted) and doc-1 "Branching and upstream sync" (guide, with branch map, integration, upstream-sync and verification steps).

Changed files: AGENTS.md (new Ramas section with links to decision-1 and doc-1), README.md (short Ramas note linking AGENTS.md and decision-1), plus the generated backlog/ task, decision and doc files. No product code and no groma/ files changed.

Verification: `backlog task list --plain`, `backlog milestone list`, `backlog decision list`, `backlog doc list` and `groma agent-instructions` succeeded; `git diff --check` clean; `git status --short` showed only AGENTS.md, README.md and backlog/. `pnpm check` is not applicable because the repo has no package.json (pnpm exits 1 with ERR_PNPM_NO_PKG_MANIFEST); no Rust code changed.

No merge, no push, no local main created, and no product-code changes. Follow-up when the operator approves: `git switch celld-os/rpc && git merge --ff-only celld/backlog-groma-adoption`.
<!-- SECTION:FINAL_SUMMARY:END -->
