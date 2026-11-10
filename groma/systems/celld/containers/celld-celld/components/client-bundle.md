---
type: C4 Component
title: Bundle
status: stable
groma:
  id: client-bundle
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/ltx/src/client/bundle.rs
  group: Storage and replication
description: ReplicaClient overlay that exposes bundle-resident L0 segments.
---
