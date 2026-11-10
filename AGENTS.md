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

## Ramas

- `upstream/main` es el repo original de denoland. Solo se usa para sync.
- `origin/main` es el espejo del fork; no es base de trabajo ni de deploy.
- `celld-os/rpc` es la rama de integración de `1us/celld-os`. Es la base para
  features, fixes y para el branch de adopción.
- `celld/backlog-groma-adoption` se integra en `celld-os/rpc`, nunca en main.
- Las ramas nuevas salen de `celld-os/rpc`:
  `git switch -c <tipo>/<tema> celld-os/rpc`.
- El sync con upstream se hace sobre `celld-os/rpc`, no sobre main.
- Detalle: [decision-1](backlog/decisions/decision-1%20-%20Integration-branch-is-celld-os-rpc.md)
  + [doc-1](backlog/docs/doc-1%20-%20Branching-and-upstream-sync.md).

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

## Trabajo (Backlog) y arquitectura (Groma)

- El trabajo planificado vive en `backlog/` (Backlog.md, prefijo `CELLD`). Antes de empezar
  algo, buscá con `backlog search` / `backlog task list`; antes de crear, empezar o cerrar una
  tarea, leé `backlog instructions task-creation`, `task-execution` y `task-finalization`.
  Usá el CLI `backlog`; no edites los archivos de tareas a mano.
- Los registros de diseño, límites operativos y hallazgos quedan en `docs/`; no son tareas.
  Una tarea los enlaza con `--ref`.
- La migración del trabajo previo está en `docs/planning-migration.md`.
- La arquitectura vive en `groma/` y se edita solo con el CLI `groma`.
- Los links de arquitectura se agregan con `--add-ref <element-id|file>`; usá el
  ID real o el archivo exacto, nunca un ID inventado.
- `--doc` y `--ref` son contexto: no sustituyen un link de arquitectura.
- Registrá `--modified-file` después de cada cambio; las tareas de curación
  listan todos los paths de `groma/` que cambiaron.

<!-- groma:start -->
## Groma

This project uses Groma. Before you scan, inspect, or curate architecture, or change files for a Backlog task, run `groma agent-instructions` and read the guide it names for that job. When it reports a first scan, ask the user whether they want you to curate the architecture. Do not edit Groma-owned architecture files directly.
<!-- groma:end -->
