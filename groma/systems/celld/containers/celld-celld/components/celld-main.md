---
type: C4 Component
title: Main
status: stable
groma:
  id: celld-main
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/celld/main.rs
  group: Cell runtime
description: 'Entrypoint of the celld node: startup, listeners, shutdown and the V8 shell.'
---
