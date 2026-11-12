---
id: CELLD-7
title: Disposing a foreign capability does not send a remote release
status: Done
assignee: []
created_date: '2026-11-08T16:23:00Z'
updated_date: '2026-11-12T01:37:00Z'
labels: []
dependencies: []
references:
  - docs/PR-READY.md
  - docs/RPC-CROSS-ISOLATE.md
  - capability
  - cell-dispatch
modified_files:
  - crates/celld/capability.rs
  - docs/guarantees.md
  - docs/PR-READY.md
  - docs/RPC-CROSS-ISOLATE.md
priority: low
type: task
ordinal: 7000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Disposing a capability held by another isolate does not send a remote release; the owner entry is reclaimed only with its cell or isolate. Decide whether to send an explicit remote release or to document the lazy reclamation as the contract. Migrated from docs/PR-READY.md (Known limits).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Remote release behavior is implemented or documented as the intended contract
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Decision: la reclamacion perezosa ES el contrato; no se envia release remoto cross-node.

Mecanismo verificado leyendo el codigo, no asumido: CapabilityRefs es un static por proceso (js.rs capability_refs); recibir una capability sobre RPC NO toma entrada (solo dup(), capabilityToken() y re-exportar lo hacen, harness.js __stubLift); y op_cap_release solo envia el dispose cuando release() devuelve Some(true), que en el receptor es None por clave ausente. Por eso el dispose en el receptor es inerte y la entrada del owner se reclama con su celda/isolate.

Descartado el conteo fleet-wide: requeriria tabla compartida mas un lease por handle, con liveness resuelta por el mismo CAS de bucket que ya reclama un owner perdido, para liberar una entrada barata que muere con su isolate.

Contrato agregado en docs/guarantees.md en la seccion "Reclaiming a capability's owner entry" (incluye la regla observable: disposed / owner-gone / owner-moved). RPC-CROSS-ISOLATE.md y PR-READY.md dejan de presentarlo como pendiente.

Test: capability::tests::releasing_a_capability_a_process_never_retained_is_inert fija el mecanismo (tabla por proceso, release inerte, owner intacto).

Verificacion: cargo test -p celld --lib capability 21/21; cargo test --workspace 68 passed / 0 fallas / 15 resultados ok; clippy -D warnings limpio; rustfmt y git diff --check limpios. Sin cambios de runtime.
<!-- SECTION:NOTES:END -->
