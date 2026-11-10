---
type: C4 Component
title: Http streams
status: stable
groma:
  id: http-streams
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/celld/http_streams.rs
  group: RPC and networking
description: 'The HTTP stream registry: request and response bodies that cross the host boundary by id rather than by value.'
---
