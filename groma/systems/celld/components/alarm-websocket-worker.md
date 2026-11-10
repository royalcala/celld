---
type: C4 Component
title: Worker
status: stable
groma:
  id: alarm-websocket-worker
  parent: celld
  code:
    - scanner: javascript
      file: crates/celld/tests/fixtures/alarm-websocket/worker.js
      symbol: AlarmSocket
  group: Test fixtures
description: Test fixture Worker for alarms and hibernating WebSockets.
---
