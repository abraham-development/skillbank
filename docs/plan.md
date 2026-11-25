# Plan: Banco de Skills (`skillbank`)

## Contexto

El usuario crea Agent Skills (carpetas con `SKILL.md`) mientras trabaja con agentes de IA (Claude Code, Codex, Cursor, Antigravity), y hoy se quedan atrapadas en cada proyecto. Quiere un **banco personal** en su laptop (Ubuntu 26.04) donde:
- un agente, con lenguaje natural ("lleva esta skill a mi banco"), la suba ejecutando un CLI;
- en otro proyecto, otro agente la traiga ("trae la skill X del banco");
- desde una app de escritorio pueda ver, buscar, crear y editar skills a mano.

Carpeta del proyecto: `/home/abraham2/Proyectos/Banco de Skills` (vacía, sin git). Rust no está instalado; Node 22 y pnpm sí.

## Decisiones tomadas con el usuario

| Tema | Decisión |
|---|---|
| Stack | Tauri 2 (2.12, webkit2gtk-4.1) + Rust ≥ 1.90; frontend React + TS + Vite |
| CLI | binario `skillbank`; comparte el crate núcleo con la app (la app no llama al CLI) |
| Skill que ya existe al subir | Sobrescribir con confirmación, sin historial. CLI: exit 5 `CONFLICT` con resumen de diferencias; solo con `--force` sobrescribe. Red de seguridad: papelera local 7 días + papelera de Drive |
| Traer a un proyecto | Copia independiente (nunca symlinks; nunca escribir *a través* de un symlink) |
| Agentes | Claude Code, Codex, Cursor, Antigravity (rutas en config, editables) |
| Conexión con agentes | CLI + skill guía `banco-de-skills` instalada en la carpeta global de cada agente. Sin MCP en v1 |
| Funciones v1 | Etiquetas + búsqueda; editor con vista previa y validación en vivo; importar/exportar .zip; registro de instalaciones con detección de copias desactualizadas |
| Respaldo | Google Drive con `rclone bisync` (≥ 1.66, NO el de apt que es 1.60) |
| Validación | Estándar Agent Skills obligatorio; campos solo-Claude-Code permitidos con aviso |
| Idioma | UI y mensajes en español; claves JSON y códigos de error en inglés (estables) |

## Arquitectura

Workspace Cargo (edition 2024, `rust-version = "1.90"`):

```
Banco de Skills/
├── Cargo.toml  rust-toolchain.toml  package.json  vite.config.ts  AGENTS.md  README.md
├── docs/{spec-validator.md, cli-json-schema.md, e2e-checklist.md}
├── scripts/{install-cli.sh, gen-completions.sh}
├── crates/
│   ├── skillbank-core/          # TODA la lógica
│   │   ├── assets/guide/banco-de-skills/{SKILL.md, references/comandos.md}   # embebida
│   │   ├── assets/templates/{basica,con-scripts,con-referencias}/SKILL.md
│   │   └── src/{paths,config,lock,fsutil,hash,meta,bank,diff,search,agents,
│   │            registry,install,archive,sync,guide,doctor}.rs
│   │            skill/{frontmatter,validate,links}.rs
│   └── skillbank-cli/src/{main,cli,output}.rs + cmd/*.rs   # clap, --json, exit codes
├── src/                         # React: features/{library,skill,editor,wizard,install,import-export,sync,settings}
└── src-tauri/src/{lib,state,error,watcher,autosync}.rs + commands/*.rs
```

- `src-tauri` entra al workspace recién en la Fase 2 (`default-members = ["crates/*"]`), así `cargo test` no exige las librerías -dev de webkit.
- Crates: serde/serde_json, **`yaml_serde` 0.10** (serde_yaml está archivado, serde_yml tiene RUSTSEC-2025-0068), toml, thiserror/anyhow, walkdir, tempfile, sha2, zip 9 (deflate), similar, pulldown-cmark, chrono, uuid, directories, deunicode, clap + clap_complete. App: tauri 2.12, plugins dialog/opener/single-instance, notify-debouncer-full, ts-rs. Bloqueo con `std::fs::File::lock` (estable desde 1.89; sin fs2).
- Frontend: react-query, Tailwind v4 + shadcn/ui, `@uiw/react-codemirror` (markdown + yaml frontmatter + lint), react-markdown + remark-gfm. Tipos generados con ts-rs.

## Almacenamiento

```
~/.config/skillbank/config.toml          # ajustes + tabla de agentes (no se sincroniza)
~/.local/share/skillbank/bank/           # ← lo ÚNICO que sincroniza rclone (ruta configurable)
    bank.json  RCLONE_TEST
    skills/<name>/SKILL.md, scripts/, references/, assets/   # idénticas a lo que se instala
    meta/<name>.json                     # etiquetas, fechas, origen, hash, bits +x
~/.local/share/skillbank/{staging,trash}/   # reemplazo atómico; papelera local 7 días
~/.local/state/skillbank/{bank.lock, installs.json, sync.json, logs/}   # solo local
```

- **Metadatos en `meta/<name>.json` aparte** (no dentro de SKILL.md ni dentro de la carpeta de la skill): así la skill es byte a byte la misma que se instala y exporta, y el hash coincide.
- **Sin SQLite en v1**: búsqueda en memoria (sin acentos, por nombre/descripción/contenido/etiquetas).
- **Hash de contenido** sha256 determinista del árbol (ruta + contenido; ignora `.git`, `__pycache__`, `*.pyc`, `*.swp`, `*.sbconflict*`, `__MACOSX`; no cuenta permisos ni fechas). Invariante: `hash(copia) == hash(origen)`.
- **Registro de instalaciones local** (rutas absolutas solo valen en esta máquina; reconstruible con `skillbank scan`). Estados comparando disco (D), instalado (I) y banco (B): `al día`, `desactualizada`, `modificada localmente`, `divergente`, `falta`, `huérfana`.
- **Concurrencia**: lock exclusivo para escrituras y sync, compartido para copiar hacia afuera, sin lock para lecturas; reemplazo vía staging + rename; editor con hash esperado (conflicto → Recargar/Comparar/Sobrescribir); la GUI se refresca con un watcher (evento `bank-changed`).

## Agentes (defaults en config.toml)

| Agente | Proyecto | Global | También lee |
|---|---|---|---|
| Claude Code | `.claude/skills` | `~/.claude/skills` (nunca escribir en `synced/`) | — |
| Codex | `.agents/skills` | `~/.agents/skills` | `~/.codex/skills` (legado) |
| Cursor | `.cursor/skills` | `~/.cursor/skills` | `.agents`, `.claude`, `.codex` (proyecto y global) |
| Antigravity | `.agents/skills` | `~/.gemini/config/skills` | `.agent/skills`, `~/.gemini/antigravity/skills` |

- Elección de agente: `--agent X` (repetible, `all`, `detected`) → variable de entorno (`CLAUDECODE=1`) → marcadores del proyecto → `default_agents`.
- **Deduplicación**: no copiar a una carpeta si los agentes que la necesitan ya leen otra del plan (p. ej. {claude, cursor} → solo `.claude/skills`), para que Cursor no vea la skill dos veces. Raíz de proyecto = `.git` más cercano o cwd.

## CLI `skillbank` (nunca pregunta; todo lo destructivo pide `--force`/`--yes`)

Flags globales: `--json`, `-q`, `-v`, `--bank`, `--config`, `--lock-timeout`.

| Comando | Qué hace |
|---|---|
| `push <ruta\|nombre> [--force] [--tags] [--dry-run] [--diff]` | valida y sube; `created` / `unchanged` / `updated`; exit 5 con diff si ya existe y difiere |
| `pull <nombre>... [--agent] [--project DIR \| --global] [--force] [--dry-run]` | copia a las carpetas del/los agente(s); actualiza sola una copia no modificada; exit 5 si hay cambios locales |
| `list [--tag]`, `search <q>`, `show <n> [--body]`, `path <n>` | consultar |
| `validate <ruta\|n> [--profile open\|claude-ai\|claude-code] [--strict]` | validar |
| `new`, `remove --yes`, `tag`, `rename` | gestionar |
| `export <n>... [-o] [--claude-ai] [--strip-nonstandard] \| --all`, `import <zip\|dir>` | zip (protección zip-slip, límites 50 MB / 2000 entradas) |
| `installed`, `outdated [--update]`, `scan [--register] [--import]` | registro de instalaciones |
| `sync [--status] [--dry-run] [--resync] [--conflicts] [--install-timer]` | Drive |
| `agents list\|detect\|install-guide`, `config`, `doctor [--fix]`, `completions` | utilidades |

**Salida `--json`**: un único objeto `{schema, ok, command, code, exit_code, message, hint, data, warnings}`; en conflicto, `data` trae `diff` (archivos, claves del frontmatter, parche de SKILL.md truncado) y `relation`.

**Códigos de salida**: 0 OK · 1 ERROR · 2 USAGE · 3 NOT_FOUND (`did_you_mean`) · 4 INVALID · 5 CONFLICT · 6 LOCKED · 7 SYNC_UNAVAILABLE · 8 SYNC_FAILED · 9 AMBIGUOUS.

El CLI nunca usa la red: las escrituras marcan `dirty=true` y la sincronización la hacen la GUI o el timer.

## Skill guía `banco-de-skills` (la que conecta a los agentes)

Va embebida en el binario y se instala con `skillbank agents install-guide --all` en `~/.claude/skills/`, `~/.agents/skills/` y `~/.gemini/config/skills/` (Cursor queda cubierto por las dos primeras). Lleva `metadata.skillbank-guide-version` para avisar cuando esté desactualizada. Su descripción incluye frases en español e inglés ("sube esta skill a mi banco", "trae la skill X", "push this skill to my skill bank"…).

Reglas para el agente:
1. Usar siempre `--json` y `--agent <yo>`.
2. **Nunca `--force` sin un "sí" explícito del usuario**, después de mostrarle el resumen de cambios.
3. Nunca `remove` si el usuario no lo pidió.
4. No escribir en `synced/`.
5. Nunca correr `rclone config`.

Flujos: validar → push → manejar el exit 5; buscar → pull → reportar la ruta; outdated. Incluye la tabla de códigos y qué hacer ante un sandbox. Los detalles van en `references/comandos.md`.

## Validador (un solo motor en Rust para CLI y GUI)

- **Errores**:
  - falta SKILL.md, no es UTF-8, frontmatter ausente, sin cerrar o con YAML inválido;
  - `name`: 1–64 caracteres, `[a-z0-9-]`, sin guiones al borde ni `--`, igual a la carpeta, sin nombres reservados (`synced`, `anthropic-skills`, `banco-de-skills`);
  - `description`: 1–1024 caracteres;
  - `compatibility`: 1–500 caracteres;
  - `metadata` debe ser string→string; tipos incorrectos (p. ej. `name: no`).
- **Perfil claude-ai** (estricto): además son errores las claves fuera de las 6 estándar, "anthropic"/"claude" en el nombre y etiquetas XML.
- **Avisos**:
  - campos solo-Claude-Code (etiqueta "solo Claude Code", más fuerte si el destino es otro agente);
  - claves desconocidas;
  - description + when_to_use > 1536;
  - cuerpo > 500 líneas o ≈5000 tokens;
  - referencias > 100 líneas sin índice, referencias anidadas, enlaces rotos, rutas con `\`;
  - archivos de conflicto; archivos grandes.
- **Info**: la descripción no dice "cuándo usarla".
- Admite BOM y CRLF. Los mensajes indican línea y columna.

## GUI (español)

1. **Biblioteca**: búsqueda (Ctrl+K), filtros por etiqueta, Inválidas, Con advertencias, Desactualizadas; tarjetas con insignias; indicador de sincronización.
2. **Detalle**: pestañas Vista / Archivos / Instalaciones / Validación; acciones Editar, Instalar en proyecto, Exportar, Exportar para claude.ai, Renombrar, Eliminar.
3. **Editor**:
   - árbol de archivos, CodeMirror con lint en vivo (Rust, cada 250 ms) y vista previa;
   - contadores de líneas, tokens y descripción;
   - Ctrl+S para guardar; banner si hay un cambio externo.
4. **Nueva skill**: plantilla → título con slug automático → descripción con contador → etiquetas → estándar o con campos de Claude Code.
5. **Instalar en proyecto**: carpeta o Global, agentes detectados, **vista previa del plan** (destinos, deduplicación, avisos, diff si hay conflicto).
6. **Instalaciones**: por proyecto, con estados; "Actualizar todas"; "Escanear proyecto".
7. **Importar**: desde .zip, carpeta o "Escanear agentes", con estado nueva / igual / distinta por fila. **Exportar**: .zip estándar, claude.ai o respaldo completo.
8. **Sincronización**:
   - asistente inicial: rclone ≥ 1.66 → remote configurado → prueba → simulación → confirmar;
   - registro en vivo y panel de conflictos.
9. **Ajustes**: ruta del banco; rutas por agente; guía por agente (instalar o actualizar); CLI en el PATH; sincronización; diagnóstico.

Tauri: comandos `async` con `spawn_blocking` que llaman al núcleo. Capacidades mínimas (`core:default`, `dialog:allow-open`, `dialog:allow-save`), sin plugins fs/shell/http. CSP estricta. Una sola instancia de la app.

## Sincronización con Drive

- Comando base: `rclone bisync <banco> gdrive:BancoDeSkills` con `--resilient --recover --max-lock 2m --conflict-resolve newer --conflict-suffix sbconflict --check-access --compare size,modtime,checksum --drive-skip-gdocs`.
- **Primera vez**:
  - remoto vacío → `--resync-mode path1`;
  - banco local vacío (máquina nueva) → `path2`;
  - ambos con contenido → solo manual, tras una simulación.
- **Automática solo con la GUI abierta**: al iniciar, 60 s después de un cambio, cada 15 min y al cerrar. Opcional: timer systemd de usuario (`sync --install-timer`).
- La sincronización automática **nunca** usa `--resync` ni `--force`. Conflictos: archivo `.sbconflictN`, visible y resoluble desde CLI y GUI.

## Fases

**Fase 0 — Preparación**: rustup, `git init`, workspace solo con core + cli, AGENTS.md. *Listo cuando*: `cargo test` pasa y `skillbank --version` funciona.

**Fase 1a — Núcleo + CLI mínimo (los agentes ya pueden usarlo)**:
- frontmatter, validador, hash, lock, meta, bank;
- `push`, `pull`, `list`, `search`, `show`, `path`, `validate`, `remove`;
- `--json` y códigos de salida;
- agentes con deduplicación y registro básico;
- skill guía + `install-guide`;
- `scripts/install-cli.sh` (→ `~/.local/bin/skillbank`).

*Listo cuando*:
- hay ≥ 45 casos de validador y pruebas de todos los códigos de salida;
- **prueba real con Claude Code**: en el proyecto A, "sube la skill X" → aparece en `list`; editarla y volver a subirla → exit 5 → Claude pregunta, y solo tras un "sí" usa `--force`; en el proyecto B, "trae la skill X" → `.claude/skills/X` funciona en la misma sesión;
- Codex trae la skill a `.agents/skills`.

**Fase 1b — Resto del CLI**: `installed`, `outdated`, `scan`, `tag`, `rename`, `new`, `import`/`export`, `doctor`, `completions`. *Listo cuando*:
- hay pruebas de la matriz de estados y de zip-slip / zip bomb;
- un zip `--claude-ai` sube bien a claude.ai (prueba manual);
- `scan --global --import` trae las skills actuales de `~/.claude/skills` y `~/.agents/skills` sin duplicar los symlinks.

**Fase 2 — App de escritorio**:
- 2a: paquetes apt + scaffold Tauri + Biblioteca y Detalle + watcher;
- 2b: editor, asistente, etiquetas;
- 2c: instalar, instalaciones, importar/exportar, ajustes;
- 2d: `.deb` con `/usr/bin/skillbank` y autocompletado.

*Listo cuando*:
- un `push` desde el CLI aparece en la GUI en menos de 1 s;
- editar en la GUI mientras el CLI sobrescribe muestra el banner de conflicto;
- `apt install ./banco-de-skills_*.deb` deja el CLI y la entrada en el menú.

**Fase 3 — Sincronización con Drive**: `core::sync`, `skillbank sync`, pantalla de Sincronización, sincronización automática. *Listo cuando*:
- pasan las pruebas con un rclone falso y con un bisync real entre dos carpetas temporales;
- prueba manual con Drive usando un segundo banco (`--bank /tmp/bank2`): el conflicto aparece y se puede resolver;
- sin rclone da exit 7 con guía en pantalla.

**Pendiente para después (no v1)**: servidor MCP, historial de versiones, bandeja del sistema, AppImage.

## Pasos que debe correr el usuario

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh          # Fase 0
sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file pkg-config \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev       # Fase 2
sudo -v ; curl https://rclone.org/install.sh | sudo bash                 # Fase 3 (≥ 1.66)
rclone config    # remote "gdrive", tipo drive, scope drive.file          # Fase 3
```

Si la ventana sale en blanco: `WEBKIT_DISABLE_DMABUF_RENDERER=1`.

## Verificación

- `cargo test --workspace`:
  - validador por tabla (BOM, CRLF, `name: no`, límites 64/65 y 1024/1025, `Synced`, perfil claude-ai);
  - hash y copia; deduplicación de agentes; estados del registro; zip;
  - CLI con assert_cmd en directorios temporales (`HOME` y `SKILLBANK_*`): push → pull, conflicto → `--force`, destino symlink, lock ocupado → 6, registro sin permisos → aviso.
- Checklist manual en `docs/e2e-checklist.md`:
  - Claude Code, Codex y Cursor (sin duplicados en el selector);
  - zip a claude.ai;
  - GUI + CLI a la vez;
  - sincronización con Drive (primera vez, conflicto, sin conexión);
  - instalar y quitar el .deb.
