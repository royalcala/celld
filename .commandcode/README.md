# `.commandcode` project state

Tracked:

- `taste/` — reviewed project preferences and conventions.
- `skills/` — project-specific skills, when present.

Local only (ignored):

- `settings.json` / `settings.local.json` — may contain machine paths or permissive modes.
- `sessions/`, `logs/`, `cache/`, `history.jsonl`, `auth.json`.

Do not commit secrets, credentials, session logs, or raw plan dumps. If a setting is meant to be shared, add a sanitized `settings.example.json` instead. An already-tracked, sanitized `settings.json` is an explicit exception; do not add machine paths or permissive modes.
