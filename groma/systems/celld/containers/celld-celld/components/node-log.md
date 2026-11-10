---
type: C4 Component
title: Node log
status: stable
groma:
  id: node-log
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/celld/node_log.rs
  group: Fleet operations
description: v0 of the in-fleet replicated log tier (CELLD_DURABILITY=fleet).
---
