---
type: C4 Component
title: Wake
status: stable
groma:
  id: celld-wake
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/celld/wake.rs
  group: Storage and replication
description: Immutable alarm discovery publications and restart-safe reclamation.
---
