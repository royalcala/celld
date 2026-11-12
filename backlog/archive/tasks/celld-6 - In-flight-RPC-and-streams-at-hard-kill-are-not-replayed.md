---
id: CELLD-6
title: In-flight RPC and streams at hard kill are not replayed
status: Done
assignee: []
created_date: '2026-11-08T16:23:00Z'
updated_date: '2026-11-12T01:21:00Z'
labels: []
dependencies: []
references:
  - docs/PR-READY.md
  - docs/guarantees.md
  - docs/RPC-CROSS-ISOLATE.md
  - capability
  - peer-tunnel
  - http-streams
modified_files:
  - crates/celld/js.rs
  - docs/guarantees.md
  - docs/PR-READY.md
  - docs/RPC-CROSS-ISOLATE.md
priority: low
type: task
ordinal: 6000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
An in-flight RPC or stream at the instant of a hard node kill is ambiguous: the runtime does not replay it. Decide whether the client re-acquires or the runtime offers a bounded replay/repair path, and document the chosen behavior. Migrated from docs/PR-READY.md (Known limits).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The hard-kill in-flight behavior is decided and documented, with a test if the runtime changes
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Decision: el cliente re-adquiere; el runtime NO replaya. Fundamento: el runtime no puede probar que el handler no commiteo antes del kill, asi que replay = side effect duplicado; settlear con error es la unica respuesta honesta.

Contrato documentado en docs/guarantees.md -> 'In-flight calls at a hard kill': respuesta settled = hecho; llamada unsettled = ambigua (transport, u owner-moved si la celda se movio); retry es decision de la app (idempotency key/upsert/lectura); un nodo fenced no reanuda la llamada y el sucesor activa a epoch mayor.

Test nuevo: js::rpc_error_tests::a_hard_kill_surfaces_as_an_ambiguous_error fija los dos caminos (clasificador de mensajes del peer y sitios locales que fijan el codigo) y descarta success/plain failure.

El test encontro un detalle real: 'stub RPC owner dropped its result' llega como codigo directo 'transport' desde el dispatch local, no via classify_owner_error (que solo lee mensajes del peer); el test quedo alineado a eso.

Verificacion: cargo test -p celld --lib rpc_error_tests 3/3; cargo test --workspace 67 passed / 0 fallas / 15 resultados ok; clippy -D warnings limpio; rustfmt limpio; git diff --check limpio.

PR-READY.md y RPC-CROSS-ISOLATE.md dejan de listarlo como limit abierto y enlazan la garantia.
<!-- SECTION:NOTES:END -->
