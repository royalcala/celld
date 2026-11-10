---
type: C4 Component
title: Wal
status: stable
groma:
  id: wal
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/ltx/src/wal.rs
  group: Storage and replication
description: SQLite WAL header/frame parsing + cumulative SQLite checksums.
---
