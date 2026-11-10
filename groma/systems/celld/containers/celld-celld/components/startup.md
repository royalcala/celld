---
type: C4 Component
title: Startup
status: stable
groma:
  id: startup
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/celld/startup.rs
  group: Fleet operations
description: Process startup primitives shared by the standalone and eventual managed celld adapters.
---
