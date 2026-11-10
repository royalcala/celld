---
type: C4 Component
title: Control plane
status: stable
groma:
  id: control-plane
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/celld/control_plane.rs
  group: Fleet operations
description: Managed control-plane sessions that run outside the actor domain.
---
