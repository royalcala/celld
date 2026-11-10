---
type: C4 Component
title: Cell host
status: stable
groma:
  id: cell-host
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/celld/cell_host.rs
  group: Cell runtime
description: The Actor's one boundary to whichever engine runs cells.
---
