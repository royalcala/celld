---
type: C4 Container
title: celld node
status: stable
groma:
  id: celld-celld
  parent: celld
  technology: Rust, V8, SQLite, Tokio, Hyper
description: The celld daemon that serves Workers and Durable Objects on one machine.
---

One celld process is a node. It hosts the public and internal listeners, the V8 isolate pool, the cell actor runtime, replication to the fleet bucket, peer tunnels and the operator API. crates/celld is the effect executor and adapter host; crates/logic holds behavioral decisions; crates/ltx implements SQLite log replication.
