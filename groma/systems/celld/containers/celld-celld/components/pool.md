---
type: C4 Component
title: Pool
status: stable
groma:
  id: pool
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/celld/pool.rs
  group: Cell runtime
description: 'The isolate pool: the shell around celld_logic::isolate.'
---
