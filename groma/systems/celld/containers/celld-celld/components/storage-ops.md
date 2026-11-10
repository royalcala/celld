---
type: C4 Component
title: Storage ops
status: stable
groma:
  id: storage-ops
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/celld/js/storage_ops.rs
  group: Storage and replication
description: 'The V8 surface over crate::storage: key-value, SQL, and the value encoding shared by both.'
---
