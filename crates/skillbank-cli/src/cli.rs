//! Definición de argumentos (clap). Ayuda en español.

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

const AFTER_HELP: &str = "\
Ejemplos:
  skillbank push .claude/skills/mi-skill --json      Sube una skill al banco
  skillbank pull mi-skill --agent claude --json      La trae al proyecto actual
  skillbank search \"pdf\"                             Busca en el banco
  skillbank agents install-guide                     Enseña a tus agentes a usar el banco

Códigos de salida: 0 OK · 1 error · 2 uso · 3 no encontrada · 4 inválida · 5 conflicto
                   6 banco ocupado · 7 sync no disponible · 8 sync falló · 9 ambigua";

#[derive(Debug, Parser)]
#[command(
    name = "skillbank",
    version,
    about = "Banco de Skills: guarda y reutiliza tus Agent Skills entre proyectos y agentes",
    after_help = AFTER_HELP,
    disable_help_subcommand = true
)]
pub struct Cli {
    /// Salida como un único objeto JSON (recomendado para agentes).
    #[arg(long, global = true)]
    pub json: bool,
    /// Menos salida en modo humano.
    #[arg(short, long, global = true)]
    pub quiet: bool,
    /// Más detalle en modo humano.
    #[arg(short, long, global = true)]
    pub verbose: bool,
    /// Usa otro banco (por defecto ~/.local/share/skillbank/bank).
    #[arg(long, global = true, value_name = "DIR")]
    pub bank: Option<PathBuf>,
    /// Archivo de configuración (por defecto ~/.config/skillbank/config.toml).
    #[arg(long, global = true, value_name = "ARCHIVO")]
    pub config: Option<PathBuf>,
    /// Segundos de espera si el banco está ocupado.
    #[arg(long, global = true, default_value_t = 30, value_name = "SEG")]
    pub lock_timeout: u64,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Sube una skill al banco (crea o actualiza).
    Push(PushArgs),
    /// Trae skills del banco a este proyecto o a la carpeta global de un agente.
    Pull(PullArgs),
    /// Lista las skills del banco.
    #[command(visible_alias = "ls")]
    List(ListArgs),
    /// Busca skills por nombre, etiquetas, descripción y contenido.
    Search(SearchArgs),
    /// Muestra una skill del banco.
    Show(ShowArgs),
    /// Imprime la ruta de una skill dentro del banco.
    Path {
        /// Nombre de la skill.
        name: String,
    },
    /// Valida una skill (carpeta o nombre en el banco) contra la especificación.
    Validate(ValidateArgs),
    /// Elimina una skill del banco (va a la papelera local 7 días).
    #[command(visible_alias = "rm")]
    Remove(RemoveArgs),
    /// Agentes: rutas, detección e instalación de la skill guía.
    Agents {
        #[command(subcommand)]
        cmd: AgentsCmd,
    },
    /// Configuración y rutas.
    Config {
        #[command(subcommand)]
        cmd: ConfigCmd,
    },
    /// Genera el autocompletado para tu shell.
    Completions {
        /// bash, zsh, fish, elvish o powershell.
        shell: clap_complete::Shell,
    },
}

#[derive(Debug, Args)]
pub struct PushArgs {
    /// Carpeta de la skill, su SKILL.md o su nombre (se busca en las carpetas de los agentes).
    pub target: String,
    /// Agente que hace la subida: claude, codex, cursor o antigravity.
    #[arg(long, value_name = "AGENTE")]
    pub agent: Option<String>,
    /// Etiquetas separadas por comas (reemplazan las actuales).
    #[arg(long, value_name = "A,B")]
    pub tags: Option<String>,
    /// Añade las etiquetas de --tags en vez de reemplazarlas.
    #[arg(long, requires = "tags")]
    pub add_tags: bool,
    /// Sobrescribe la versión del banco si es distinta. Solo con permiso del usuario.
    #[arg(long)]
    pub force: bool,
    /// Muestra qué pasaría sin escribir nada.
    #[arg(long)]
    pub dry_run: bool,
    /// Incluye el parche completo de SKILL.md si hay conflicto.
    #[arg(long)]
    pub diff: bool,
    /// Permite subir con errores de validación (salvo errores de `name`).
    #[arg(long)]
    pub allow_invalid: bool,
}

#[derive(Debug, Args)]
pub struct PullArgs {
    /// Nombre(s) de la(s) skill(s).
    #[arg(required = true)]
    pub names: Vec<String>,
    /// Agente(s) destino (repetible): claude, codex, cursor, antigravity, all, detected.
    #[arg(long, value_name = "AGENTE")]
    pub agent: Vec<String>,
    /// Carpeta del proyecto (por defecto, el directorio actual).
    #[arg(long, value_name = "DIR", conflicts_with = "global")]
    pub project: Option<PathBuf>,
    /// Instala en la carpeta global del agente en vez del proyecto.
    #[arg(long)]
    pub global: bool,
    /// Sobrescribe copias locales distintas. Solo con permiso del usuario.
    #[arg(long)]
    pub force: bool,
    /// Muestra qué pasaría sin escribir nada.
    #[arg(long)]
    pub dry_run: bool,
    /// Copia en la carpeta principal de cada agente aunque otro ya la lea.
    #[arg(long)]
    pub no_dedupe: bool,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Solo las skills con esta etiqueta.
    #[arg(long)]
    pub tag: Option<String>,
    /// Solo las skills con errores de validación.
    #[arg(long)]
    pub invalid: bool,
}

#[derive(Debug, Args)]
pub struct SearchArgs {
    /// Texto a buscar (todas las palabras deben aparecer).
    pub query: String,
    /// Solo las skills con esta etiqueta.
    #[arg(long)]
    pub tag: Option<String>,
    /// Máximo de resultados.
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
}

#[derive(Debug, Args)]
pub struct ShowArgs {
    /// Nombre de la skill.
    pub name: String,
    /// Incluye el cuerpo de SKILL.md.
    #[arg(long)]
    pub body: bool,
}

#[derive(Debug, Args)]
pub struct ValidateArgs {
    /// Carpeta de la skill, su SKILL.md o el nombre de una skill del banco.
    pub target: String,
    /// open (estándar), claude-ai (estricto) o claude-code.
    #[arg(long, default_value = "open")]
    pub profile: String,
    /// Agente(s) destino: avisa de campos que ese agente ignora.
    #[arg(long, value_name = "AGENTE")]
    pub agent: Vec<String>,
    /// Los avisos también hacen fallar la validación.
    #[arg(long)]
    pub strict: bool,
}

#[derive(Debug, Args)]
pub struct RemoveArgs {
    /// Nombre de la skill.
    pub name: String,
    /// Confirma la eliminación (obligatorio).
    #[arg(long)]
    pub yes: bool,
}

#[derive(Debug, Subcommand)]
pub enum AgentsCmd {
    /// Rutas de cada agente y si está instalado.
    List,
    /// Agentes detectados en un proyecto y plan de instalación.
    Detect {
        /// Carpeta del proyecto (por defecto, el directorio actual).
        dir: Option<PathBuf>,
    },
    /// Instala (o quita) la skill guía `banco-de-skills` en los agentes.
    InstallGuide {
        /// Agente(s): claude, codex, cursor, antigravity.
        #[arg(long, value_name = "AGENTE", conflicts_with = "all")]
        agent: Vec<String>,
        /// Todos los agentes, estén instalados o no.
        #[arg(long)]
        all: bool,
        /// Reemplaza una skill ajena con el mismo nombre.
        #[arg(long)]
        force: bool,
        /// Quita la guía en vez de instalarla.
        #[arg(long)]
        uninstall: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigCmd {
    /// Rutas de configuración, banco y estado.
    Path,
    /// Muestra la configuración efectiva (TOML).
    Show,
}
