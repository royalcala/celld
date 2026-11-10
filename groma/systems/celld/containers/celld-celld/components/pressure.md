---
type: C4 Component
title: Pressure
status: stable
groma:
  id: pressure
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/logic/pressure.rs
  group: Fleet operations
description: Memory-pressure shedding as a pure classifier of a memory sample plus the prior shedding state (the hysteresis latch).
---
