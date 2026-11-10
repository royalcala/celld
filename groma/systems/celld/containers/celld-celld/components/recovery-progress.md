---
type: C4 Component
title: Recovery progress
status: stable
groma:
  id: recovery-progress
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/celld/node_log/recovery_progress.rs
  group: Storage and replication
description: Durable hints for the cell epochs a recovery has already folded.
---
