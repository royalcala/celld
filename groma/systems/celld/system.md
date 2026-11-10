---
type: C4 System
title: celld
status: stable
groma:
  id: celld
  technology: Rust, V8, SQLite, S3-compatible object storage
description: Self-hosted Workers and Durable Objects runtime.
---

celld executes a Cloudflare Workers application as a fleet of nodes that share one object-store bucket. Each Durable Object is a cell: a single-threaded actor with its own SQLite database, ownership lease and replication log. The system covers cell execution, durability and failover, service bindings and RPC, the bundled KV, Queues, D1, R2, Workflow, Cron and asset services, and the operator CLI.
