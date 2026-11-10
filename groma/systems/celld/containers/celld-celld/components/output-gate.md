---
type: C4 Component
title: Output gate
status: stable
groma:
  id: output-gate
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/logic/output_gate.rs
  group: Cell runtime
description: 'The output gate: one choke point every egress passes through.'
---
