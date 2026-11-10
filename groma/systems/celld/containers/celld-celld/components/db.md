---
type: C4 Component
title: Db
status: stable
groma:
  id: db
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/ltx/src/db.rs
  group: Storage and replication
description: SQLite lifecycle, WAL capture and checkpointing for replication.
---
