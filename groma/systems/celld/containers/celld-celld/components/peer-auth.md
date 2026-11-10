---
type: C4 Component
title: Peer auth
status: stable
groma:
  id: peer-auth
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/celld/peer_auth.rs
  group: RPC and networking
description: Fleet peer authentication, signed requests and replay protection.
---
