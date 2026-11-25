# Banco de Skills

Tu banco personal de **Agent Skills**. Creas una skill trabajando con Claude Code, Codex,
Cursor o Antigravity, le dices al agente *"sube esta skill a mi banco"*, y en cualquier otro
proyecto le dices *"trae la skill X de mi banco"*. Funciona con el estándar abierto
[Agent Skills](https://agentskills.io/specification).

> Estado: **Fase 1 completa** — núcleo + CLI `skillbank` listos (incluye importar/exportar,
> registro de instalaciones y diagnóstico). La app de escritorio, que aparecerá en *Mostrar
> aplicaciones* de Ubuntu (Fase 2), y la sincronización con Google Drive (Fase 3) vienen después.
> Ver [docs/plan.md](docs/plan.md).

## Instalación

```bash
# Requiere Rust (rustup). Instala ~/.local/bin/skillbank y la skill guía en tus agentes:
scripts/install-cli.sh
```

La **skill guía** `banco-de-skills` se instala en `~/.claude/skills/` y `~/.agents/skills/`
(y `~/.gemini/config/skills/` si usas Antigravity). Es lo que hace que tus agentes entiendan
"sube/trae una skill de mi banco" en lenguaje natural.

## Uso rápido

```bash
skillbank push .claude/skills/mi-skill --tags pdf     # subir (o por nombre: skillbank push mi-skill)
skillbank pull mi-skill                               # traer al proyecto actual
skillbank pull mi-skill --agent codex --global        # traer a la carpeta global de Codex
skillbank list                                        # ver el banco
skillbank search "facturas pdf"                       # buscar
skillbank show mi-skill --body                        # ver una skill
skillbank validate ./mi-skill --profile claude-ai     # ¿se puede subir a claude.ai?
skillbank agents list                                 # rutas y estado de cada agente
```

Más comandos:

```bash
skillbank new mi-skill --template con-scripts --dir .claude/skills -d "Qué hace. Úsala cuando…"
skillbank tag mi-skill --add pdf,documentos           # etiquetas (sin nombre: lista todas)
skillbank rename mi-skill pdf-extractor               # renombrar en el banco
skillbank export mi-skill --claude-ai --strip-nonstandard -o ~/Descargas   # zip para claude.ai
skillbank export --all -o ~/Respaldos                 # respaldo completo del banco
skillbank import respaldo.zip --skip-existing         # importar zip o carpeta
skillbank installed mi-skill                          # dónde está instalada
skillbank outdated --project . --update               # actualizar copias sin cambios locales
skillbank scan --global                               # skills de tus agentes que no están en el banco
skillbank scan --global --import                      # … y subirlas
skillbank doctor --fix                                # revisar y arreglar lo seguro
```

Todos los comandos aceptan `--json` (un único objeto con `ok`, `code`, `exit_code`,
`message`, `hint`, `data`, `warnings`). Ver [docs/cli-json.md](docs/cli-json.md).

### Reglas importantes

- **Subir una skill que ya existe y es distinta** falla con código 5 (`CONFLICT`) y muestra
  las diferencias. Solo se sobrescribe con `--force` (la versión anterior va a la papelera
  local 7 días).
- **Traer** copia la carpeta (nunca enlaces). Si la copia del proyecto no se tocó desde que
  se instaló, se actualiza sola; si tiene cambios locales, es un conflicto.
- Si eliges varios agentes, no se copia dos veces: Cursor también lee `.claude/skills` y
  `.agents/skills`, así que `--agent claude --agent cursor` usa solo `.claude/skills`.

## Dónde vive todo

| Qué | Ruta |
|---|---|
| Banco | `~/.local/share/skillbank/bank/` (`skills/`, `meta/`) |
| Configuración | `~/.config/skillbank/config.toml` (rutas por agente, editables) |
| Registro de instalaciones | `~/.local/state/skillbank/installs.json` |

## Carpetas por agente

| Agente | Proyecto | Global |
|---|---|---|
| Claude Code | `.claude/skills` | `~/.claude/skills` |
| Codex | `.agents/skills` | `~/.agents/skills` |
| Cursor | `.cursor/skills` | `~/.cursor/skills` |
| Antigravity | `.agents/skills` | `~/.gemini/config/skills` |

## Desarrollo

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
```

Ver [AGENTS.md](AGENTS.md) para las convenciones del repositorio.
