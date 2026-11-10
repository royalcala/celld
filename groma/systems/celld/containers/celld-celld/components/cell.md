---
type: C4 Component
title: Cell
status: stable
groma:
  id: cell
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/logic/cell.rs
      symbol: valid_cell_scope
  group: Cell runtime
description: Cell-scope predicates, reified sans-IO.
---
