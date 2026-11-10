---
type: C4 Component
title: Drain token
status: stable
groma:
  id: drain-token
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/celld/drain_token.rs
  group: Fleet operations
description: 'The fleet drain token: one bucket object that serializes donors.'
---
