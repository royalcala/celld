# AGENTS — celld

`celld` es el runtime self-hosted de Durable Objects: Workers, DOs/SQLite,
KV, Queues, D1, R2, Workflows, Cron, assets, dynamic workers y containers
experimentales. **No es una app de producto ni la plataforma 1us.**

Si sos una sesión nueva, leé esto y los docs enlazados antes de tocar código.

## Ecosistema

- `1us-v4` — plataforma: infra, flotas, Tailscale, R2, secrets.
- `celld-void` — adapter/tooling para desplegar apps Void en celld.
- `celld-os` — OS/Workshop (gadgets, agentes, gatekeepers) sobre celld.
- `1us-contracts`, `1us-databases` — apps de producto.
- Este repo es el **runtime**; no mezclar cambios de apps ni infra.

## Estado de trabajo

- Rama actual: `celld-os/rpc`.
- Línea de trabajo: RPC cross-isolate, streams, `ctx.restore`, hooks y
  persistencia de stubs. `celld-os` depende de esto.
- No commitear ni mezclar cambios de producto en esta rama sin coordinación.

## Leer primero

- `README.md`
- `docs/guarantees.md`
- `docs/cloudflare-compat.md`
- `docs/limitations.md`
- `docs/RPC-CROSS-ISOLATE.md`
- `docs/services/*.md`
- `docs/testing.md`
- `docs/security.md`

## Reglas

- Rust; cambios chicos y con tests.
- No tocar `1us-v4/infra`, `1us-secrets` ni flotas.
- No agregar código de producto/apps al runtime.
- Respetar compatibilidad Cloudflare documentada; si falta algo, documentarlo
  en `docs/cloudflare-compat.md`/`docs/limitations.md`.
- Migraciones/protocolos: no romper guarantees; tests de durabilidad,
  hibernación, restart, takeover y fleet.

## Comandos

```bash
cargo fmt --check
cargo clippy --all-targets
cargo test
```

Los tests de integración/E2E están en `docs/testing.md`.
