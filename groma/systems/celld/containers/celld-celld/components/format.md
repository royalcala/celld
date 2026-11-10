---
type: C4 Component
title: Format
status: stable
groma:
  id: format
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/logic/format.rs
      symbol: fleet_reads
  group: Storage and replication
description: The bucket format a node can read, and the fleet-wide gate that keeps a node from writing what a live peer cannot read.
---
