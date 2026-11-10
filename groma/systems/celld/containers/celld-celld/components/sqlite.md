---
type: C4 Component
title: Sqlite
status: stable
groma:
  id: sqlite
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/logic/sqlite.rs
      symbol: poisons_actor
  group: Storage and replication
description: SQLite failure handling, reified sans-IO.
---
