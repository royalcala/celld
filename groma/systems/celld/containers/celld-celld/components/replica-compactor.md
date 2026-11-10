---
type: C4 Component
title: Replica compactor
status: stable
groma:
  id: replica-compactor
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/ltx/src/replica_compactor.rs
  group: Storage and replication
description: Additive LTX compaction through a [ReplicaClient].
---
