---
type: C4 Component
title: Host channels
status: stable
groma:
  id: host-channels
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/celld/host_channels.rs
  group: RPC and networking
description: 'The channels a cell runtime uses to hand work to the host: one request type and one sender per host service.'
---
