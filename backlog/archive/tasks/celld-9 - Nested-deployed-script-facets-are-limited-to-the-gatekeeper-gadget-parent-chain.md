---
id: CELLD-9
title: >-
  Nested deployed-script facets are limited to the gatekeeper/gadget parent
  chain
status: Done
assignee: []
created_date: '2026-11-08T16:23:00Z'
updated_date: '2026-11-12T01:51:00Z'
labels: []
dependencies: []
references:
  - docs/PR-READY.md
  - docs/limitations.md
  - docs/RPC-CROSS-ISOLATE.md
  - cell-dispatch
  - facet-streams
  - harness
modified_files:
  - crates/celld/storage.rs
  - docs/limitations.md
  - docs/PR-READY.md
  - docs/RPC-CROSS-ISOLATE.md
priority: low
type: task
ordinal: 9000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Nested deployed-script facets only work in the gatekeeper/gadget parent-chain shape. Decide whether to generalize nesting or to document the constraint in docs/limitations.md. Migrated from docs/PR-READY.md (Known limits).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The nesting limit is generalized or documented as an explicit limitation
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Decision: el limite se documenta como limitacion explicita; no se generaliza el nesting entre roots.

Hecho verificado en el codigo, no asumido:
- storage.rs open_embedded: ensure!(parent.facet_path.len() < 3), o sea root (0) mas 3 facetas = profundidad total 4. El guard corre antes de tocar la base de la faceta.
- bootstrap.rs: la profundidad es parent.facet_path.len() + 1, es decir la longitud del camino de nombres bajo el root.
- harness.js: un ctx.restore() en una faceta solo puede mintear por un stub padre creado con ctx.restore(); el row del root guarda {parent, params} y el replay reabre la cadena nivel por nivel con __restoreEntry + __facetInvokers. Esa cadena cuelga de UN root, asi que no cruza entre dos scripts desplegados. Cuando los datos durables pertenecen a otro script, la via es su propio Durable Object via service binding.

Generalizar el nesting entre roots exigiria persistir el loader/clase de cada nivel y un punto de replay compartido entre dos raices, lo que agrega estado y acoplamiento para una forma que no se usa. Se documenta el limite y la alternativa.

Docs: docs/limitations.md gana una seccion "Facets" con el tope de profundidad y la regla de cadena; RPC-CROSS-ISOLATE.md y PR-READY.md apuntan ahi y dejan de tratarlo como pendiente. De paso corregi en RPC-CROSS-ISOLATE.md la entrada de /metrics, que seguia diciendo que no habia politica (CELLD-8).

Test: storage::tests::a_fourth_nested_facet_is_refused fija el tope (profundidades 0..3 pasan el guard; 3 se rechaza). Counter-proof: relajando el guard a < 9 el test falla; restaurado, verde.

Verificacion: cargo test -p celld --lib storage::tests 2/2; cargo test --workspace 69 passed / 0 fallas / 15 resultados ok; clippy -D warnings limpio; rustfmt y git diff --check limpios. Sin cambios de runtime (solo docs y un test).
<!-- SECTION:NOTES:END -->
