# Referencia de comandos de `skillbank`

## Contenido

- [Opciones globales](#opciones-globales)
- [push](#push)
- [pull](#pull)
- [list, search, show, path](#list-search-show-path)
- [validate](#validate)
- [remove](#remove)
- [tag, rename, new](#tag-rename-new)
- [export, import](#export-import)
- [installed, outdated, scan](#installed-outdated-scan)
- [doctor](#doctor)
- [agents](#agents)
- [Formato JSON](#formato-json)

## Opciones globales

| Opción | Efecto |
|---|---|
| `--json` | Un único objeto JSON por stdout (recomendado para agentes). |
| `-q`, `--quiet` | Menos salida en modo humano. |
| `--bank <dir>` | Usa otro banco (pruebas). |
| `--lock-timeout <s>` | Segundos de espera si el banco está ocupado (por defecto 30). |

## push

`skillbank push <ruta|nombre> [--agent X] [--tags a,b] [--add-tags] [--force] [--dry-run] [--diff] [--allow-invalid]`

- `<ruta>`: carpeta de la skill o su `SKILL.md`. Con un nombre, se busca en las carpetas
  de skills del proyecto y globales de los agentes.
- El banco guarda la skill con el `name` del frontmatter.
- `--tags` reemplaza las etiquetas; con `--add-tags` se añaden.
- `--dry-run` muestra qué pasaría sin escribir.
- `--diff` incluye el parche completo de SKILL.md en el conflicto.
- `--force` sobrescribe la versión del banco. **Solo con permiso explícito del usuario.**

## pull

`skillbank pull <nombre>... [--agent X]... [--project <dir> | --global] [--force] [--dry-run] [--no-dedupe]`

- Destino por defecto: la raíz del proyecto actual (el `.git` más cercano).
- `--agent` se puede repetir y admite `all` y `detected`. Sin `--agent`, se usa el agente que
  ejecuta el comando (si se detecta), luego los marcadores del proyecto y luego la configuración.
- Carpetas por agente (proyecto / global):
  - Claude Code: `.claude/skills` / `~/.claude/skills`
  - Codex: `.agents/skills` / `~/.agents/skills`
  - Cursor: `.cursor/skills` / `~/.cursor/skills` (también lee `.agents` y `.claude`)
  - Antigravity: `.agents/skills` / `~/.gemini/config/skills`
- Sin `--no-dedupe`, no se copia dos veces si un agente ya lee otra carpeta del plan.
- Si la copia local no se tocó desde que se instaló, se actualiza sola (sin `--force`).

## list, search, show, path

- `skillbank list [--tag T] [--json]`
- `skillbank search "<texto>" [--tag T] [--limit N] [--json]`
- `skillbank show <nombre> [--body] [--json]`
- `skillbank path <nombre>`

## validate

`skillbank validate <ruta|nombre> [--profile open|claude-ai|claude-code] [--agent X] [--strict] [--json]`

- `open` (por defecto): estándar Agent Skills. Los campos solo-Claude-Code son informativos.
- `claude-ai`: estricto; solo los 6 campos estándar (lo que acepta la subida a claude.ai).
- `--strict`: los avisos también hacen fallar la validación.

## remove

`skillbank remove <nombre> --yes` — mueve la skill a la papelera local (7 días).
**Solo si el usuario lo pidió.**

## tag, rename, new

- `skillbank tag [<nombre>] [--add a,b] [--remove a] [--set a,b]` — sin nombre, lista las etiquetas.
- `skillbank rename <viejo> <nuevo>` — cambia carpeta, `name` y metadatos. Las copias ya
  instaladas conservan el nombre anterior.
- `skillbank new <nombre> [--template basica|con-scripts|con-referencias] [-d "…"] [--dir DIR | --in-bank] [--tags a,b]`

## export, import

- `skillbank export <nombre>... [-o archivo.zip|carpeta] [--claude-ai] [--strip-nonstandard]`
  — un `.zip` por skill con `<nombre>/SKILL.md…` (formato de claude.ai).
- `skillbank export --all [-o carpeta]` — respaldo completo (`bank.json`, `skills/`, `meta/`).
- `skillbank import <zip|carpeta> [--skip-existing | --force] [--only a,b] [--tags a,b] [--dry-run]`
  — si hay versiones distintas en el banco, no importa nada sin `--skip-existing` o `--force`.
  Rechaza zips con rutas peligrosas, enlaces simbólicos, más de 2000 entradas o más de 50 MB.

## installed, outdated, scan

- `skillbank installed [<nombre>] [--project DIR]` — dónde está instalada cada skill.
- `skillbank outdated [--project DIR] [--update]` — estados: `up_to_date`, `outdated`
  (se actualiza sola con `--update`), `modified` (cambios locales), `diverged`, `missing`, `orphan`.
- `skillbank scan [DIR | --global] [--register] [--import]` — compara las skills de las
  carpetas de los agentes con el banco (`same`, `different`, `absent`, `invalid`).
  `--register` anota las idénticas como instaladas; `--import` sube las que faltan.

## doctor

`skillbank doctor [--fix]` — revisa banco, CLI en el PATH, guía, metadatos, registro y rclone.
`--fix` solo aplica arreglos seguros.

## agents

- `skillbank agents list` — rutas de cada agente y si está instalado.
- `skillbank agents detect [dir]` — agentes detectados en un proyecto.
- `skillbank agents install-guide [--agent X | --all] [--force] [--uninstall]` — instala esta guía.

## Formato JSON

```json
{
  "schema": 1, "ok": false, "command": "push", "code": "CONFLICT", "exit_code": 5,
  "message": "…", "hint": "…",
  "data": { "name": "…", "relation": "fast_forward", "diff": { "files": [ … ] } },
  "warnings": [ … ]
}
```
