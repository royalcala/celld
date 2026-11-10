---
type: C4 Component
title: Cache
status: stable
groma:
  id: logic-cache
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/logic/cache.rs
      symbol: plan_eviction
  group: Storage and replication
description: Local snapshot-cache eviction, sans-IO.
---
