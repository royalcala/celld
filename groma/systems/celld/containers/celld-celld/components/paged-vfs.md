---
type: C4 Component
title: Paged vfs
status: stable
groma:
  id: paged-vfs
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/ltx/src/paged_vfs.rs
  group: Storage and replication
description: SQLite VFS that faults database pages in from the bucket.
---
