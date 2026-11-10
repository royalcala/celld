---
type: C4 Component
title: Queue batching
status: stable
groma:
  id: queue-batching
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/celld/queue_batching.rs
      symbol: timing
  group: Durable Object services
description: Producer and fleet-log batching timing for Queues.
---
