# Salida JSON de `skillbank`

Con `--json`, cada comando imprime **un único objeto** por stdout (las claves salen en este orden):

```json
{
  "schema": 1,
  "ok": true,
  "command": "push",
  "code": "OK",
  "exit_code": 0,
  "message": "✓ Skill 'pdf-tools' subida al banco (nueva).",
  "data": { },
  "warnings": [ ]
}
```

En error, `ok` es `false` y se añade `hint` (qué hacer). `data` trae el detalle del error.

## Códigos de salida

| exit | `code` | Cuándo |
|---|---|---|
| 0 | `OK` | Éxito, incluido "sin cambios". |
| 1 | `ERROR` | Error inesperado o de E/S. |
| 2 | `USAGE` | Argumentos incorrectos; `remove` sin `--yes`. |
| 3 | `NOT_FOUND` | Skill o ruta inexistente. `data.did_you_mean`. |
| 4 | `INVALID` | Errores de validación. `data.diagnostics`. |
| 5 | `CONFLICT` | Ya existe y es distinta. Pedir permiso y repetir con `--force`. |
| 6 | `LOCKED` | Banco ocupado (otra operación o sincronización). Reintentar. |
| 7 | `SYNC_UNAVAILABLE` | rclone ausente/antiguo o remoto sin configurar (Fase 3). |
| 8 | `SYNC_FAILED` | Falló la sincronización (Fase 3). |
| 9 | `AMBIGUOUS` | Varias copias distintas con ese nombre. `data.candidates`. |

## `data` por comando

### push

```json
{ "name": "pdf-tools", "status": "created|updated|unchanged", "dry_run": false,
  "source": "/ruta/origen", "bank_path": "/ruta/banco/skills/pdf-tools",
  "hash": "sha256:…", "previous_hash": "sha256:…", "tags": ["pdf"],
  "warnings": [Diagnostic], "registered": true, "registry_warning": null }
```

Conflicto (exit 5):

```json
{ "name": "pdf-tools", "source": "…", "bank_hash": "sha256:…", "source_hash": "sha256:…",
  "relation": "fast_forward|bank_newer|diverged|unrelated",
  "diff": { "files": [{"path": "SKILL.md", "status": "modified", "added": 1, "removed": 0}],
            "frontmatter": [{"key": "description", "change": "modified"}],
            "skill_md_patch": "--- banco/SKILL.md\n+++ local/SKILL.md\n…", "patch_truncated": false } }
```

| `relation` | Significado |
|---|---|
| `fast_forward` | El banco no cambió desde que se instaló aquí: solo se añadirían los cambios locales. |
| `bank_newer` | La copia local no cambió pero el banco sí: sobrescribir revertiría el banco. |
| `diverged` | Ambas cambiaron desde la instalación. |
| `unrelated` | No hay registro de esta copia. |

### pull

```json
{ "scope": "project|global", "project_root": "/proyecto",
  "agents": ["claude", "cursor"], "agent_source": "explicit|environment|project_markers|config_default|all",
  "destinations": [{"dir": "/proyecto/.claude/skills", "agents": ["claude", "cursor"]}],
  "items": [{ "skill": "pdf-tools", "path": "/proyecto/.claude/skills/pdf-tools",
              "agents": ["claude", "cursor"], "action": "installed|updated|unchanged|conflict",
              "reason": "symlink|modified|unregistered|not_a_dir|forced",
              "local_hash": "…", "bank_hash": "…", "diff": { } }],
  "warnings": [Diagnostic], "dry_run": false }
```

En conflicto (exit 5) **no se escribe nada** y `data` es este mismo objeto.

### list / search / show / validate

- `list`: lista de `{name, description, tags, created_at, updated_at, content_hash, valid, errors, warnings, claude_code_fields, path}`.
- `search`: lista de `{name, description, tags, score, matched_in, snippet}`.
- `show`: resumen + `frontmatter`, `body` (con `--body`), `files`, `diagnostics`, `meta`, `installs[{path, scope, agents, status}]`.
- `validate`: `{path, profile, valid, report: {name, description, frontmatter, body_lines, body_tokens_est, claude_code_fields, diagnostics}}`.

### Diagnostic

```json
{ "code": "E_NAME_CHARS", "severity": "error|warning|info", "message": "…",
  "file": "SKILL.md", "line": 2, "field": "name", "tag": "solo Claude Code", "hint": "…" }
```

Los códigos están en [spec-validator.md](spec-validator.md).
