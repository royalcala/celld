#!/usr/bin/env bash
# Inspect an upstream celld release and merge it into a new sync branch.
#
# Usage: scripts/upstream-sync.sh vX.Y.Z [--inspect-only]
#
# This script never pushes, tags or publishes. It leaves conflicts in the
# working tree for a human to resolve per docs/UPSTREAM-SYNC.md.
set -euo pipefail
cd "$(dirname "$0")/.."

tag=${1:-}
mode=${2:-}
if [ -z "$tag" ]; then
  echo "usage: $0 vX.Y.Z [--inspect-only]" >&2
  exit 2
fi

git fetch origin --tags
git fetch upstream --tags
git rev-parse --verify "${tag}^{commit}" >/dev/null

base=$(git merge-base HEAD "$tag")
echo "upstream tag: $tag ($(git rev-parse --short "${tag}^{}"))"
echo "base:         $base"
echo "range:        $base..$tag"
git log --oneline "$base".."$tag"
git diff --stat "$base".."$tag"

if [ "$mode" = "--inspect-only" ]; then
  exit 0
fi

# Agent/session metadata does not belong to a merge decision.
if [ -n "$(git status --porcelain -- . ':!.commandcode')" ]; then
  echo "working tree is not clean; commit or stash first" >&2
  exit 1
fi

branch="upstream-sync/${tag}"
git switch -c "$branch"
if git merge --no-ff --no-edit "$tag" -m "merge: upstream celld $tag"; then
  echo
  echo "clean merge on $branch; run the matrix in docs/UPSTREAM-SYNC.md"
else
  echo
  echo "conflicts on $branch; resolve per docs/UPSTREAM-SYNC.md, then commit" >&2
  exit 1
fi
