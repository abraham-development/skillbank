# Banco de Skills — guía para agentes que trabajan en este repositorio

App de escritorio (Tauri 2 + React) y CLI (`skillbank`) para guardar Agent Skills
(carpetas con `SKILL.md`) en un banco personal y reutilizarlas entre proyectos y agentes
(Claude Code, Codex, Cursor, Antigravity). Plataforma objetivo: Ubuntu 26.04.

## Estructura

- `crates/skillbank-core/` — **toda la lógica**: validador (`skill/validate.rs`), banco
  (`bank.rs`), hash (`hash.rs`), agentes y deduplicación (`agents.rs`), push/pull
  (`install.rs`), registro de instalaciones (`registry.rs`), skill guía (`guide.rs` +
  `assets/guide/`). La app y el CLI no duplican lógica: la llaman desde aquí.
- `crates/skillbank-cli/` — el binario `skillbank` (clap). `cmd.rs` adapta el núcleo a
  la salida humana/JSON de `output.rs`.
- `src-tauri/` y `src/` — app de escritorio (Fase 2, aún no existe).
- `docs/` — reglas del validador, formato JSON del CLI, checklist manual.

## Comandos

```bash
cargo test --workspace                      # todas las pruebas
cargo clippy --workspace --all-targets      # debe quedar en 0 avisos
cargo fmt --all
scripts/install-cli.sh                      # instala ~/.local/bin/skillbank + guía
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
- El plan del proyecto (fases y decisiones) está en `docs/plan.md`.
