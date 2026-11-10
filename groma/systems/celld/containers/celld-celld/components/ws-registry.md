---
type: C4 Component
title: Ws registry
status: stable
groma:
  id: ws-registry
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/celld/ws_registry.rs
  group: RPC and networking
description: 'The WebSocket registry: the sockets an instance holds, the frames waiting to leave, and the isolate-polled input queues.'
---
