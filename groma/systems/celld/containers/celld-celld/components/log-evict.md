---
type: C4 Component
title: Log evict
status: stable
groma:
  id: log-evict
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/logic/log_evict.rs
  group: Storage and replication
description: Sans-IO gray-follower eviction policy for the replicated log tier.
---
