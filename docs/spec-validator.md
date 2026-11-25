# Reglas del validador

Fuentes: [especificación Agent Skills](https://agentskills.io/specification),
[Claude Code: Skills](https://code.claude.com/docs/en/skills) y
[buenas prácticas de la plataforma Claude](https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices).

Perfiles: `open` (por defecto, estándar), `claude-ai` (estricto: lo que acepta la subida a
claude.ai y la Skills API) y `claude-code` (no avisa de los campos propios de Claude Code).

## Estructura y frontmatter

| Código | Severidad | Regla |
|---|---|---|
| `E_SKILLMD_MISSING` | error | No hay `SKILL.md` (pista si existe `skill.md`). |
| `E_UTF8` | error | `SKILL.md` no es UTF-8. |
| `W_BOM` | aviso | `SKILL.md` empieza con BOM. |
| `E_FM_MISSING` | error | La primera línea no es `---`. |
| `E_FM_UNCLOSED` | error | Falta el `---` de cierre. |
| `E_FM_YAML` | error | YAML inválido (claves duplicadas, `:` sin comillas…). |
| `E_FM_NOT_MAP` | error | El frontmatter no es un mapa. |
| `E_FM_KEY_TYPE` | error | Clave no textual. |

## Campos estándar

| Código | Severidad | Regla |
|---|---|---|
| `E_NAME_MISSING` / `E_NAME_TYPE` | error | `name` obligatorio y de texto. |
| `E_NAME_LEN` | error | 1–64 caracteres. |
| `E_NAME_CHARS` | error | Solo `a-z`, `0-9`, `-` (sugiere un nombre válido). |
| `E_NAME_HYPHEN` | error | Sin guion al inicio/fin ni `--`. |
| `E_NAME_DIR_MISMATCH` | error (aviso al subir) | `name` igual a la carpeta. Al subir, el banco guarda por `name`. |
| `E_NAME_RESERVED` | error | `synced`, `anthropic-skills`, `banco-de-skills` (sin distinguir mayúsculas). |
| `C_RESERVED_WORD` | aviso / error en claude-ai | `name` contiene "anthropic" o "claude". |
| `C_XML_TAGS` | aviso / error en claude-ai | Etiquetas XML en `name` o `description`. |
| `E_DESC_MISSING` / `E_DESC_TYPE` / `E_DESC_EMPTY` | error | `description` obligatoria, de texto, no vacía. |
| `E_DESC_LEN` | error | Máximo 1024 caracteres. |
| `I_DESC_NO_WHEN` | info | La descripción no dice cuándo usar la skill. |
| `E_LICENSE_TYPE` | error | `license` de texto. |
| `E_COMPAT_TYPE` / `E_COMPAT_LEN` | error | `compatibility` de texto, 1–500 caracteres. |
| `E_METADATA_TYPE` | error | `metadata` debe ser un mapa. |
| `W_METADATA_VALUE` | aviso | Valores de `metadata` que no son texto. |
| `E_ALLOWED_TOOLS_TYPE` | error | `allowed-tools` de texto. |
| `W_ALLOWED_TOOLS_LIST` | aviso / error en claude-ai | `allowed-tools` como lista (solo Claude Code). |

## Campos no estándar

| Código | Severidad | Regla |
|---|---|---|
| `I_CC_ONLY` | info, etiqueta "solo Claude Code" | `when_to_use`, `argument-hint`, `arguments`, `disable-model-invocation`, `user-invocable`, `disallowed-tools`, `model`, `effort`, `context`, `agent`, `background`, `hooks`, `paths`, `shell`. |
| `W_CC_ONLY_TARGET` | aviso | Igual, pero el agente destino lo ignora (Cursor entiende `paths` y `disable-model-invocation`). |
| `C_NONSTANDARD_KEY` | error (solo claude-ai) | Cualquier clave fuera de las 6 estándar. |
| `W_UNKNOWN_KEY` | aviso | Clave que no es del estándar ni de Claude Code. |
| `W_CC_LISTING_TRUNC` | aviso | `description` + `when_to_use` > 1536 (Claude Code los recorta). |

## Cuerpo y archivos

| Código | Severidad | Regla |
|---|---|---|
| `W_BODY_EMPTY` | aviso | Cuerpo vacío. |
| `W_BODY_LINES` | aviso | Más de 500 líneas. |
| `W_BODY_TOKENS` | aviso | Más de ≈5000 tokens (caracteres / 4). |
| `W_BROKEN_LINK` | aviso | Enlace relativo a un archivo inexistente. |
| `W_BACKSLASH_PATH` | aviso | Ruta con `\`. |
| `W_NESTED_REFS` | aviso | Una referencia enlaza a otra (mantener un solo nivel). |
| `W_REF_NO_TOC` | aviso | `.md` de más de 100 líneas sin índice al inicio. |
| `W_LARGE_FILE` / `W_LARGE_SKILL` | aviso | Archivo > 5 MB / skill > 20 MB. |
| `W_CONFLICT_FILE` | aviso | Archivo `*.sbconflict*` de la sincronización. |

Nota: `yaml_serde` sigue YAML 1.2, así que `name: no` se lee como el texto `"no"`
(no hay "problema Noruega"); `name: true` o `name: 123` sí dan `E_NAME_TYPE`.
