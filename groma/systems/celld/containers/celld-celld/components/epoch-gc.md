---
type: C4 Component
title: Epoch gc
status: stable
groma:
  id: epoch-gc
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/logic/epoch_gc.rs
  group: Storage and replication
description: Which superseded LTX epoch prefixes an owner may delete.
---
