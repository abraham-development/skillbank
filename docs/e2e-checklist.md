# Checklist manual de punta a punta

Marca cada punto al probarlo con agentes reales.

## Fase 1a — CLI con agentes

- [ ] `scripts/install-cli.sh` instala `~/.local/bin/skillbank` y la guía en los agentes detectados.
- [ ] `skillbank agents list` muestra la guía como `current` en Claude Code, Codex y Cursor.
- [ ] **Claude Code, proyecto A:** "sube la skill X a mi banco" → ejecuta `skillbank push … --json` y X aparece en `skillbank list`.
- [ ] Editar X en A y pedir de nuevo "súbela" → exit 5 → Claude resume los cambios y **pregunta**; solo tras "sí" usa `--force`.
- [ ] **Claude Code, proyecto B:** "trae la skill X de mi banco" → aparece `.claude/skills/X` y se puede usar en la misma sesión.
- [ ] Editar X en B y pedir "tráela otra vez" → conflicto `modified`; Claude ofrece subir primero los cambios.
- [ ] **Codex:** "trae la skill X" → `.agents/skills/X`. Si el sandbox bloquea `~/.local`, pide aprobación.
- [ ] **Cursor:** "trae la skill X" → aparece una sola vez en el selector de skills.
- [ ] **Antigravity** (cuando esté instalado): `skillbank agents install-guide --agent antigravity` y repetir.

## Fase 1b

- [ ] Un zip `skillbank export X --claude-ai` se sube bien en claude.ai.
- [ ] `skillbank scan --global --import` trae las skills de `~/.claude/skills` y `~/.agents/skills` sin duplicar los symlinks.

## Fase 2 — App

- [ ] Un `push` desde el CLI aparece en la app en menos de 1 s.
- [ ] Editar en la app mientras el CLI sobrescribe la misma skill muestra el aviso de conflicto.
- [ ] `sudo apt install ./banco-de-skills_*.deb` deja `/usr/bin/skillbank` y la entrada en el menú; `apt remove` limpia.

## Fase 3 — Drive

- [ ] Primera sincronización con `gdrive:BancoDeSkills`.
- [ ] Con un segundo banco (`--bank /tmp/bank2`) editando la misma skill: aparece `.sbconflict` y se resuelve.
- [ ] Sin conexión: el CLI sigue funcionando y la app muestra "cambios pendientes".
