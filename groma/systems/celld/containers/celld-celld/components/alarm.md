---
type: C4 Component
title: Alarm
status: stable
groma:
  id: alarm
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/logic/alarm.rs
      symbol: alarm_retry
  group: Durable Object services
description: Alarm retry policy, reified sans-IO.
---
