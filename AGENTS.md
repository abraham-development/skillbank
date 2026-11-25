# Banco de Skills — guía para agentes que trabajan en este repositorio

App de escritorio (Tauri 2 + React) y CLI (`skillbank`) para guardar Agent Skills
(carpetas con `SKILL.md`) en un banco personal y reutilizarlas entre proyectos y agentes
(Claude Code, Cursor y Antigravity; Codex se quitó a propósito). Plataforma objetivo: Ubuntu 26.04.

## Estructura

- `crates/skillbank-core/` — **toda la lógica**: validador (`skill/validate.rs`), banco
  (`bank.rs`), hash (`hash.rs`), agentes y deduplicación (`agents.rs`), push/pull
  (`install.rs`), registro de instalaciones (`registry.rs`), skill guía (`guide.rs` +
  `assets/guide/`). La app y el CLI no duplican lógica: la llaman desde aquí.
- `crates/skillbank-cli/` — el binario `skillbank` (clap). `cmd.rs` adapta el núcleo a
  la salida humana/JSON de `output.rs`.
- `src-tauri/` — app de escritorio (Tauri 2): `commands.rs` expone el núcleo a la interfaz,
  `watcher.rs` emite `bank-changed`/`installs-changed`/`agents-changed`.
- Conexión de agentes: `activity.rs` (núcleo) guarda en `~/.local/state/skillbank/agents.json`
  cada `skillbank connect` y cada uso identificado (`--agent`, `CLAUDECODE=1`, `CURSOR_AGENT`).
- `src/` — interfaz React + Vite + Tailwind 4 (`pages/`, `components/`, `lib/api.ts`).
  `src/dev/` es una vista previa con datos de ejemplo para el navegador (solo en `pnpm dev`).
- `docs/` — reglas del validador, formato JSON del CLI, checklist manual.

## Comandos

```bash
cargo test --workspace                      # todas las pruebas
cargo clippy --workspace --all-targets      # debe quedar en 0 avisos
cargo fmt --all
scripts/install-cli.sh                      # instala ~/.local/bin/skillbank + guía
pnpm tauri dev                              # app de escritorio en modo desarrollo
pnpm dev                                    # solo la interfaz en el navegador (datos de ejemplo)
scripts/install-desktop-entry.sh            # compila e instala la app en "Mostrar aplicaciones"
```

## Reglas

- Idioma: mensajes para personas, comentarios y docs en **español**; claves JSON, códigos
  de error/diagnóstico y nombres de código en inglés (son contrato estable).
- El CLI **nunca pregunta**: lo que sobrescribe o borra exige `--force` / `--yes`.
- Cambiar un código de salida, un código `E_*/W_*/I_*/C_*` o una clave del JSON rompe a
  los agentes: actualiza `docs/` y la guía (`assets/guide/`) y sube `GUIDE_VERSION`.
- Nunca crear symlinks al instalar; nunca escribir a través de un symlink; nunca escribir
  en `~/.claude/skills/synced/`.
- Las pruebas del CLI usan un HOME temporal (`tests/cli.rs`): no toques el HOME real.
- Para compilar la app usa `pnpm tauri build` (o el script de instalación), nunca
  `cargo build --release -p banco-de-skills`: sin la característica `custom-protocol` sale un
  binario de desarrollo que busca la interfaz en `localhost:1420`.
- `src/dev/fixtures.json` no debe contener rutas ni datos personales reales.
- En Linux la ventana se identifica como `banco-de-skills` (nombre del binario): la entrada
  `.desktop` debe llamarse igual y llevar `StartupWMClass=banco-de-skills`.
- El plan del proyecto (fases y decisiones) está en `docs/plan.md`.
