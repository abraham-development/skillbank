//! Definición de argumentos (clap). Ayuda en español.

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

const AFTER_HELP: &str = "\
Ejemplos:
  skillbank push .claude/skills/mi-skill --json      Sube una skill al banco
  skillbank connect --agent claude                   Conecta tu agente con el banco
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
    /// Agente: claude, cursor o antigravity. Identifica quién usa el CLI (la app lo muestra
    /// como conectado) y, en pull/validate/install-guide, es el destino (repetible).
    #[arg(long, global = true, value_name = "AGENTE")]
    pub agent: Vec<String>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Conecta el agente con el banco: la app de escritorio lo mostrará como conectado.
    Connect,
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
    /// Cambia las etiquetas de una skill (o lista todas las etiquetas).
    Tag(TagArgs),
    /// Renombra una skill del banco (carpeta, `name` y metadatos).
    Rename {
        /// Nombre actual.
        old: String,
        /// Nombre nuevo.
        new: String,
    },
    /// Crea una skill nueva desde una plantilla.
    New(NewArgs),
    /// Exporta skills a .zip (estándar, para claude.ai o respaldo completo).
    Export(ExportArgs),
    /// Importa skills desde un .zip o una carpeta (incluye respaldos completos).
    Import(ImportArgs),
    /// Dónde está instalada cada skill y en qué estado.
    Installed(InstalledArgs),
    /// Instalaciones que no están al día (y actualiza las seguras con --update).
    Outdated(OutdatedArgs),
    /// Busca skills en las carpetas de los agentes y las compara con el banco.
    Scan(ScanArgs),
    /// Revisa la instalación y el banco (con --fix aplica arreglos seguros).
    Doctor {
        /// Aplica los arreglos seguros.
        #[arg(long)]
        fix: bool,
    },
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
        /// Todos los agentes, estén instalados o no (si no, usa --agent o los instalados).
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

#[derive(Debug, Args)]
pub struct TagArgs {
    /// Nombre de la skill (sin él, lista todas las etiquetas del banco).
    pub name: Option<String>,
    /// Etiquetas a añadir (separadas por comas).
    #[arg(long, value_name = "A,B")]
    pub add: Vec<String>,
    /// Etiquetas a quitar.
    #[arg(long, value_name = "A,B")]
    pub remove: Vec<String>,
    /// Reemplaza todas las etiquetas.
    #[arg(long, value_name = "A,B", conflicts_with = "add")]
    pub set: Option<String>,
}

#[derive(Debug, Args)]
pub struct NewArgs {
    /// Nombre de la skill (minúsculas, números y guiones).
    pub name: String,
    /// Plantilla: basica, con-scripts o con-referencias.
    #[arg(long, default_value = "basica")]
    pub template: String,
    /// Descripción: qué hace y cuándo usarla.
    #[arg(long, short)]
    pub description: Option<String>,
    /// Etiquetas (solo con --in-bank).
    #[arg(long, value_name = "A,B")]
    pub tags: Option<String>,
    /// Carpeta donde crearla (por defecto, el directorio actual).
    #[arg(long, value_name = "DIR", conflicts_with = "in_bank")]
    pub dir: Option<PathBuf>,
    /// Crearla directamente en el banco.
    #[arg(long)]
    pub in_bank: bool,
}

#[derive(Debug, Args)]
pub struct ExportArgs {
    /// Skill(s) a exportar.
    #[arg(required_unless_present = "all")]
    pub names: Vec<String>,
    /// Respaldo completo del banco (bank.json + skills + metadatos).
    #[arg(long, conflicts_with_all = ["names", "claude_ai", "strip_nonstandard"])]
    pub all: bool,
    /// Archivo .zip o carpeta de destino (por defecto, el directorio actual).
    #[arg(short, long, value_name = "RUTA")]
    pub output: Option<PathBuf>,
    /// Comprueba que claude.ai acepte la skill (solo los 6 campos estándar).
    #[arg(long)]
    pub claude_ai: bool,
    /// Quita los campos no estándar solo en la copia exportada.
    #[arg(long)]
    pub strip_nonstandard: bool,
}

#[derive(Debug, Args)]
pub struct ImportArgs {
    /// Archivo .zip o carpeta.
    pub source: PathBuf,
    /// Sobrescribe skills del banco que sean distintas. Solo con permiso del usuario.
    #[arg(long, conflicts_with = "skip_existing")]
    pub force: bool,
    /// Importa solo las nuevas y deja las que ya existen.
    #[arg(long)]
    pub skip_existing: bool,
    /// Importa solo estas skills (separadas por comas).
    #[arg(long, value_name = "A,B")]
    pub only: Option<String>,
    /// Etiquetas a añadir a lo importado.
    #[arg(long, value_name = "A,B")]
    pub tags: Option<String>,
    /// Muestra qué pasaría sin escribir nada.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct InstalledArgs {
    /// Solo esta skill.
    pub name: Option<String>,
    /// Solo este proyecto.
    #[arg(long, value_name = "DIR")]
    pub project: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct OutdatedArgs {
    /// Solo este proyecto (usa `.` para el actual).
    #[arg(long, value_name = "DIR")]
    pub project: Option<PathBuf>,
    /// Actualiza las copias desactualizadas que no tienen cambios locales.
    #[arg(long)]
    pub update: bool,
}

#[derive(Debug, Args)]
pub struct ScanArgs {
    /// Carpeta del proyecto (por defecto, el directorio actual).
    pub dir: Option<PathBuf>,
    /// Escanea las carpetas globales de los agentes.
    #[arg(long, conflicts_with = "dir")]
    pub global: bool,
    /// Registra como instaladas las copias idénticas al banco.
    #[arg(long)]
    pub register: bool,
    /// Sube al banco las skills que no están (y las registra).
    #[arg(long)]
    pub import: bool,
}
