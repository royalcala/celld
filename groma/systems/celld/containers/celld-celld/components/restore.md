---
type: C4 Component
title: Restore
status: stable
groma:
  id: restore
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/logic/restore.rs
      symbol: previous_epoch_reusable
  group: Storage and replication
description: Restore-source durability predicates, reified sans-IO.
---
