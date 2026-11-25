---
name: banco-de-skills
description: Gestiona el banco personal de skills del usuario con el CLI `skillbank` (Banco de Skills). Guarda (push) en el banco una skill de este proyecto o de la carpeta global, trae (pull) una skill del banco a este proyecto, y lista, busca, muestra y valida skills. Úsala cuando el usuario diga "sube/guarda/lleva esta skill a mi banco", "push this skill to my skill bank", "trae la skill X de mi banco", "bring skill X from my bank", "¿qué skills tengo en el banco?", "busca en mi banco una skill para…", o mencione skillbank o Banco de Skills.
compatibility: Requiere el CLI `skillbank` en el PATH (Linux). Sirve para Claude Code, Codex, Cursor y Antigravity.
metadata:
  skillbank-guide-version: "{{GUIDE_VERSION}}"
---

# Banco de Skills

El usuario guarda sus skills reutilizables en un **banco personal** en su computadora.
La única forma correcta de leer o escribir en el banco es el CLI `skillbank`.
Nunca edites ni copies a mano archivos dentro del banco.

Si `skillbank` no está en el PATH, prueba `{{SKILLBANK_BIN}}`, `~/.local/bin/skillbank` o `/usr/bin/skillbank`.
Si tampoco existen, dile al usuario que instale Banco de Skills y detente.

## Reglas de oro

1. Usa **siempre** `--json` y lee `ok`, `code` y `exit_code` de la respuesta.
2. Identifícate con `--agent <tú>`: `claude`, `codex`, `cursor` o `antigravity`.
3. **Nunca uses `--force` sin un "sí" explícito del usuario.** Antes, muéstrale el resumen de cambios.
4. Nunca ejecutes `skillbank remove` si el usuario no lo pidió expresamente.
5. No crees enlaces simbólicos, no escribas en `~/.claude/skills/synced/` y no modifiques esta skill.
6. Nunca ejecutes `rclone config` (es interactivo): si hace falta, explícale al usuario cómo hacerlo.

## Subir una skill al banco (push)

1. Ubica la carpeta de la skill (la que contiene `SKILL.md`). Suele estar en
   `.claude/skills/<nombre>/`, `.agents/skills/<nombre>/` o `.cursor/skills/<nombre>/`,
   o en las carpetas globales equivalentes bajo `~`. También puedes pasar solo el nombre.
2. Valida: `skillbank validate <ruta> --json`. Si hay errores, propón la corrección,
   espera la aprobación del usuario, corrige y vuelve a validar.
3. Sube: `skillbank push <ruta> --agent <tú> --json` (opcional: `--tags tag1,tag2`).
4. Interpreta el resultado:
   - `status: created` o `updated` → confirma al usuario dónde quedó.
   - `status: unchanged` → el banco ya tenía exactamente esa versión.
   - **exit 5 `CONFLICT`** → ya existe una versión distinta en el banco. Resume en lenguaje
     sencillo `data.diff` (archivos cambiados, líneas +/−) y `data.relation`:
     - `fast_forward`: el banco no cambió desde que se instaló aquí; solo se añadirían tus cambios.
     - `bank_newer`: ¡ojo! el banco es más nuevo; sobrescribir lo revertiría.
     - `diverged` / `unrelated`: ambas cambiaron o no se sabe cuál es más nueva.
     Pregunta: "¿Sobrescribo la versión del banco?". **Solo si responde que sí**, repite con `--force`.
   - **exit 4 `INVALID`** → muestra `data.diagnostics` y ofrece corregir.

## Traer una skill del banco (pull)

1. Si el nombre no es exacto, busca primero: `skillbank search "<texto>" --json`.
   Si hay varias coincidencias, pregunta cuál.
2. Trae: `skillbank pull <nombre> --agent <tú> --json`. Por defecto se copia al proyecto
   actual. Usa `--global` solo si el usuario lo pide.
3. Interpreta el resultado:
   - Cada elemento de `data.items` trae `path` y `action` (`installed`, `updated` o `unchanged`).
     Dile al usuario la ruta final. Claude Code detecta la skill sin reiniciar.
   - **exit 5 `CONFLICT`** → ya hay una copia local distinta (`reason`: `modified`, `unregistered`
     o `symlink`). Si la copia local tiene cambios valiosos, ofrece subirlos primero con `push`.
     Solo con un sí explícito, repite con `--force`.
   - **exit 3 `NOT_FOUND`** → revisa `data.did_you_mean` y propón alternativas.

## Consultar

- `skillbank list --json` → todas las skills (nombre, descripción, etiquetas, validez).
- `skillbank search "<texto>" --json` → busca por nombre, etiquetas, descripción y contenido.
- `skillbank show <nombre> --body --json` → frontmatter, cuerpo, archivos y diagnósticos.
- `skillbank path <nombre>` → ruta de la skill dentro del banco (solo para leer).

## Mantener las copias al día

- `skillbank outdated --project . --json` → copias de este proyecto que no están al día
  (`status`: `outdated`, `modified`, `diverged`, `missing`, `orphan`).
- Ofrece `skillbank outdated --project . --update --json`: solo actualiza las `outdated`
  (sin cambios locales). Para `modified`/`diverged`, pregunta: ¿subir los cambios (push) o
  descartarlos (pull --force)?
- `skillbank scan --json` → skills de este proyecto que aún no están en el banco
  (`status: absent`). Ofrece subirlas; con permiso, `skillbank scan --import --json`.

## Crear y compartir

- Skill nueva: `skillbank new <nombre> --dir .claude/skills -d "Qué hace. Úsala cuando…" --json`
  (plantillas: `basica`, `con-scripts`, `con-referencias`). Edítala y luego súbela con `push`.
- Para subirla a claude.ai: `skillbank export <nombre> --claude-ai -o <carpeta> --json`.
  Si falla por campos de Claude Code, ofrece `--strip-nonstandard` (solo cambia la copia).
- Etiquetas: `skillbank tag <nombre> --add a,b --json`.

## Códigos de salida

| Código | `code` | Qué hacer |
|---|---|---|
| 0 | OK | Éxito (incluye "sin cambios"). |
| 1 | ERROR | Error inesperado: muestra `message`. |
| 2 | USAGE | Argumentos incorrectos: revisa el comando. |
| 3 | NOT_FOUND | No existe: mira `did_you_mean`. |
| 4 | INVALID | Errores de validación: corrige antes de subir. |
| 5 | CONFLICT | Pregunta al usuario; solo con un sí, usa `--force`. |
| 6 | LOCKED | El banco está ocupado (p. ej. sincronizando): reintenta en unos segundos. |
| 7 | SYNC_UNAVAILABLE | rclone o Drive no están configurados: explica cómo hacerlo. |
| 8 | SYNC_FAILED | Falló la sincronización: muestra `message` y `hint`. |
| 9 | AMBIGUOUS | Hay varias copias: pide la ruta exacta. |

## Sandbox y permisos

El banco vive en `~/.local/share/skillbank/`. Si un comando falla por permisos
(EPERM, "read-only file system"), pide al usuario aprobación para ejecutarlo fuera del sandbox.
Un aviso `REGISTRY_NOT_WRITTEN` no es un error: la operación principal sí se hizo.

Referencia completa de comandos y opciones: [references/comandos.md](references/comandos.md).
