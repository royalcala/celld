---
type: C4 Component
title: Peer tunnel
status: stable
groma:
  id: peer-tunnel
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/celld/main/peer_tunnel.rs
  group: RPC and networking
description: Pooled peer tunnel carrying forwarded cell requests as HTTP.
---
