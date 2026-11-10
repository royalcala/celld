---
type: C4 Component
title: Durability
status: stable
groma:
  id: durability
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/logic/durability.rs
      symbol: proof_deadline
  group: Storage and replication
description: When a write's durability proof gives up, sans-IO.
---
