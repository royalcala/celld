---
type: C4 Component
title: Dead node reconciliation
status: stable
groma:
  id: dead-node-reconciliation
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/logic/dead_node_reconciliation.rs
  group: Fleet operations
description: Sans-I/O decisions for retiring compatibility ownership-index debris.
---
