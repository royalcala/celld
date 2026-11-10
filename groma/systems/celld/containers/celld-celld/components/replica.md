---
type: C4 Component
title: Replica
status: stable
groma:
  id: replica
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/ltx/src/replica.rs
  group: Storage and replication
description: Single-replica sync loop and restore orchestration.
---
