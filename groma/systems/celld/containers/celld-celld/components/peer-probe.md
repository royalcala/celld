---
type: C4 Component
title: Peer probe
status: stable
groma:
  id: peer-probe
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/celld/peer_probe.rs
  group: RPC and networking
description: Challenge-bound proof that a diagnostic reached the node named by a lease.
---
