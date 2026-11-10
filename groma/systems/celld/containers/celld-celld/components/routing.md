---
type: C4 Component
title: Routing
status: stable
groma:
  id: routing
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/logic/routing.rs
      symbol: redispatch
  group: RPC and networking
description: Whether a failed peer dispatch may be re-sent.
---
