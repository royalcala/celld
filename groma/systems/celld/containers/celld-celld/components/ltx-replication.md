---
type: C4 Component
title: Ltx replication
status: stable
groma:
  id: ltx-replication
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/celld/ltx_replication.rs
  group: Storage and replication
description: The node's replication engine behind one engine-neutral handle.
---
