---
type: C4 Component
title: Kv
status: stable
groma:
  id: services-kv
  parent: celld-celld
  code:
    - scanner: javascript
      file: crates/celld/js/services/kv.js
      symbol: kvService
  group: Durable Object services
description: KV service shared by the namespace cell and the binding client.
---
