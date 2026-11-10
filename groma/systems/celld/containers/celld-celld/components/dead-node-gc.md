---
type: C4 Component
title: Dead node gc
status: stable
groma:
  id: dead-node-gc
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/celld/dead_node_gc.rs
  group: Fleet operations
description: Compatibility garbage collection for dead celld process generations.
---
