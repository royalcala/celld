---
type: C4 Component
title: Effects
status: stable
groma:
  id: effects
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/celld/actor/effects.rs
  group: Cell runtime
description: Effect dispatch keeps ordinary work off the core's lease timer thread.
---
