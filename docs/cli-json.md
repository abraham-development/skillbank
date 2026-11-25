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

### Opción global `--agent`

`--agent claude|cursor|antigravity` vale en cualquier comando. Identifica al agente que usa el CLI
(la app lo muestra como conectado y registra su actividad en `~/.local/state/skillbank/agents.json`)
y en `pull`, `validate` y `agents install-guide` también indica el destino (repetible). Sin
`--agent`, se reconoce Claude Code por `CLAUDECODE=1` y Cursor por `CURSOR_AGENT`.
`--agent codex` da `USAGE`: Codex ya no está soportado.

### connect

```json
{ "agent": "claude", "name": "Claude Code", "connected_at": "…", "first_time": true,
  "guide": "current|outdated|missing|foreign", "bank": "/…/bank" }
```

Sin agente identificado sale con `USAGE` (exit 2). Si la guía no está al día, añade el aviso
`GUIDE_NOT_CURRENT`.

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

### tag / rename / new

- `tag <n>`: `{name, tags, changed}`. Sin nombre: lista de `{tag, count}`.
- `rename`: `{old, new, path, installs_with_old_name}`.
- `new`: `{name, path, template, in_bank, files}`.

### export / import

- `export`: lista de `{name, path, files, stripped_keys, warnings}`. Con `--all`: `{path, skills}`.
- `import`: `{source, kind: "zip|dir|backup", rows, imported, updated, dry_run}` donde cada fila es
  `{name, source, status: "new|same|different|invalid|duplicate", action: "import|update|skip|conflict", diagnostics}`.
  Con alguna fila `conflict` (exit 5) **no se importa nada** y `data` es este mismo objeto.

### installed / outdated / scan

- `installed`: lista de registros `{id, skill, scope, agents, project_root, path, installed_hash, installed_at, origin, status, status_es, disk_hash, bank_hash}`.
- `outdated`: `{items: [igual que installed], updated: [rutas], registry_warning}`.
- `status`: `up_to_date`, `outdated` (actualizable sin riesgo), `modified` (cambios locales),
  `diverged` (cambiaron ambas), `missing` (la carpeta ya no existe), `orphan` (ya no está en el banco).
- `scan`: `{scope, project_root, dirs, entries, registered, imported}`; cada entrada
  `{name, path, real_path, also_at, scope, agents, synced, status: "same|different|absent|invalid", registered, action}`.

### doctor

`{checks: [{id, status: "ok|info|warn|error", message, hint, fixable, fixed}], errors, warnings, fixed}`.
Con algún `error`, el comando sale con código 1.

### Diagnostic

```json
{ "code": "E_NAME_CHARS", "severity": "error|warning|info", "message": "…",
  "file": "SKILL.md", "line": 2, "field": "name", "tag": "solo Claude Code", "hint": "…" }
```

Los códigos están en [spec-validator.md](spec-validator.md).
