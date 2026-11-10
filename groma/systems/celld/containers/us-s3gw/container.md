---
type: C4 Container
title: Test S3 gateway
status: stable
groma:
  id: us-s3gw
  parent: celld
  technology: Rust
description: Minimal S3-compatible object store used by celld fleet tests.
---

Supporting test tooling, not part of the runtime. A vendored, test-only S3 gateway whose conditional-write path is hardened so the celld ownership CAS tests are meaningful. It is excluded from the runtime workspace.
