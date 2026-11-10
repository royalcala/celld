---
type: C4 Component
title: Docker
status: stable
groma:
  id: docker
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/celld/docker.rs
  group: Deployment and secrets
description: A minimal Docker Engine API client over the daemon's unix socket.
---
