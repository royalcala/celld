---
type: C4 Component
title: Deploy
status: stable
groma:
  id: deploy
  parent: celld-celld
  code:
    - scanner: rust
      file: crates/celld/deploy.rs
  group: Deployment and secrets
description: celld deploy — build a Wrangler project and write it to the fleet bucket.
---
