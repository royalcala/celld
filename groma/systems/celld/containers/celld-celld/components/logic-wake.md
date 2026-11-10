---
type: C4 Component
title: Wake
status: stable
groma:
  id: logic-wake
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/logic/wake.rs
  group: Storage and replication
description: Immutable alarm publications and the retirement frontier, without I/O.
---
