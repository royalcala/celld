---
id: doc-1
title: Branching and upstream sync
type: guide
created_date: '2026-11-10T17:06:00Z'
updated_date: '2026-11-10T17:06:00Z'
---
# Branching and upstream sync

## Mapa de ramas

| Rama | Rol | Base |
| --- | --- | --- |
| upstream/main | Repo original de denoland/celld. Solo lectura/sync | — |
| origin/main | Espejo del fork royalcala/celld | No es base de trabajo |
| celld-os/rpc | Rama de integración de 1us/celld-os | origin/celld-os/rpc |
| celld/backlog-groma-adoption | Adopción Backlog+Groma | celld-os/rpc @ acc1a06 |

## Reglas

- La línea de trabajo de 1us/celld-os es celld-os/rpc.
- main y upstream/main son la línea original; no son base de features, fixes ni deploy.
- Las ramas nuevas salen de celld-os/rpc:
  `git switch -c <tipo>/<tema> celld-os/rpc`
- celld/backlog-groma-adoption se integra en celld-os/rpc, nunca en main.
- El sync con upstream se hace sobre celld-os/rpc, nunca sobre main.
- No se mergea la adaptación a main; no se pushea main.
- No se borran ramas de trabajo sin decisión del operador.

## Integración de la adopción

Cuando el operador lo apruebe:

```bash
git switch celld-os/rpc
git merge --ff-only celld/backlog-groma-adoption
```

El branch de adopción está 1 commit adelante de celld-os/rpc (1dde77b sobre
acc1a06), así que el fast-forward debería ser posible. No ejecutar este merge
en esta tarea.

## Sync con upstream

```bash
git fetch upstream
git switch celld-os/rpc
git merge upstream/main
```

Resolver conflictos en celld-os/rpc. No trasladar la adaptación a main.

## Nueva feature o fix

```bash
git switch celld-os/rpc
git switch -c feat/<tema>
git switch -c fix/<tema>
```

## Verificación

```bash
git branch -vv
git log --oneline --graph --decorate -10
backlog task list --plain
backlog milestone list
```
