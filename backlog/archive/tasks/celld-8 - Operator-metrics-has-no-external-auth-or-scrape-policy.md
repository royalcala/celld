---
id: CELLD-8
title: Operator /metrics has no external auth or scrape policy
status: Done
assignee: []
created_date: '2026-11-08T16:23:00Z'
updated_date: '2026-11-12T01:07:00Z'
labels: []
dependencies: []
references:
  - docs/PR-READY.md
  - docs/security.md
  - observability
  - telemetry
  - metrics
  - crates/celld/main.rs
modified_files:
  - crates/logic/metrics.rs
  - crates/logic/lib.rs
  - crates/celld/main.rs
  - crates/celld/actor.rs
  - crates/celld/tests/metrics_auth.rs
  - docs/security.md
  - docs/telemetry.md
  - docs/PR-READY.md
priority: medium
type: task
ordinal: 8000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The operator listener exposes /metrics with no authentication or scrape policy. Decide the exposure (bind scope, mTLS or proxy auth, scrape allow-list) and document it in docs/security.md. Migrated from docs/PR-READY.md (Known limits).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 /metrics exposure is authenticated or explicitly restricted, and docs/security.md records the policy
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Reificar la decisión como predicado puro en celld-logic (metrics.rs) con comparación constant-time y tests unitarios.
2. Cablear la ruta /metrics del listener interno: token opcional CELLD_METRICS_TOKEN, 401 + WWW-Authenticate, default = frontera de red.
3. Test de integración con flota real (us-s3gw + deploy) que prueba 401 anónimo, challenge, 401 con token erróneo, 200 con token y 200 anónimo en el nodo sin token; counter-proof verificando que el test falla si se saltea la política.
4. Documentar la política en docs/security.md, docs/telemetry.md y actualizar docs/PR-READY.md.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implementado en feat/metrics-auth.

Politica: /metrics conserva la frontera de red del listener interno como default. CELLD_METRICS_TOKEN agrega Authorization: Bearer <token> obligatorio; 401 + WWW-Authenticate: Bearer cuando falta, el esquema no es Bearer o el token difiere. Comparacion constant-time. Token vacio/en blanco = no configurado.

Cambios:
- crates/logic/metrics.rs (nuevo): predicado puro metrics_access + ct_eq, 4 unit tests.
- crates/celld/main.rs: internal_metrics() en la ruta /metrics, helper metrics_token(), campo AppHandle.metrics_token.
- crates/celld/actor.rs: campo metrics_token.
- crates/logic/lib.rs: modulo metrics.
- docs/security.md: seccion 'Protect the metrics endpoint'.
- docs/telemetry.md: corrige la afirmacion de endpoint sin auth.
- docs/PR-READY.md: mueve el limite a resuelto.

Verificacion:
- cargo test -p celld-logic --lib: 6/6 (4 nuevos).
- cargo test --test metrics_auth: 1/1, 2.4 s, flota real (us-s3gw + celld deploy): 401 anonimo, challenge presente, 401 con token erroneo, 200 y exposicion con token correcto, 200 anonimo sin token configurado.
- Counter-proof: con la politica bypasseada el test falla con 'an anonymous scrape must be refused, got: HTTP/1.1 200 OK'; restaurado, verde.
- cargo test --workspace: 15 resultados ok, 0 fallas, 12 binarios (incluye metrics_auth).
- cargo clippy --workspace --all-targets -- -D warnings: limpio.
- rustfmt --check sobre los archivos tocados: limpio.
- git diff --check: limpio.
<!-- SECTION:NOTES:END -->
