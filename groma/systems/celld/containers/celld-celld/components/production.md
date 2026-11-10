---
type: C4 Component
title: Production
status: stable
groma:
  id: production
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/celld/actor/production.rs
      symbol: run
  group: Cell runtime
description: The production selector for the Actor execution domain.
---
