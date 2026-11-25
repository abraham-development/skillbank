//! `skillbank`: CLI de Banco de Skills. Nunca hace preguntas interactivas: todo lo que
//! sobrescribe o borra exige `--force` / `--yes`, para que los agentes no se queden colgados.

#![allow(clippy::result_large_err)]

mod cli;
mod cmd;
mod output;

use clap::{CommandFactory, Parser};
use cli::{Cli, Command};
use output::Printer;
use skillbank_core::paths::Paths;
use skillbank_core::{Context, CoreError};
use std::time::Duration;

fn command_name(c: &Command) -> &'static str {
    match c {
        Command::Push(_) => "push",
        Command::Pull(_) => "pull",
        Command::List(_) => "list",
        Command::Search(_) => "search",
        Command::Show(_) => "show",
        Command::Path { .. } => "path",
        Command::Validate(_) => "validate",
        Command::Remove(_) => "remove",
        Command::Agents { .. } => "agents",
        Command::Config { .. } => "config",
        Command::Completions { .. } => "completions",
    }
}

fn main() {
    let wants_json = std::env::args().any(|a| a == "--json");
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            use clap::error::ErrorKind as K;
            if wants_json && !matches!(e.kind(), K::DisplayHelp | K::DisplayVersion) {
                let printer = Printer { json: true, quiet: false };
                let msg = e.to_string().lines().next().unwrap_or("argumentos inválidos").replace("error: ", "");
                let err = CoreError::usage(msg).with_hint("Ejecuta `skillbank --help` para ver el uso.");
                std::process::exit(printer.err("usage", &err));
            }
            e.exit();
        }
    };
    let printer = Printer { json: cli.json, quiet: cli.quiet };

    if let Command::Completions { shell } = &cli.command {
        let mut c = Cli::command();
        clap_complete::generate(*shell, &mut c, "skillbank", &mut std::io::stdout());
        return;
    }

    let mut paths = Paths::from_env();
    if let Some(b) = &cli.bank {
        paths.bank_override = Some(b.clone());
    }
    if let Some(c) = &cli.config {
        paths.config_file = c.clone();
    }
    let name = command_name(&cli.command);
    let ctx = match Context::load(paths) {
        Ok(mut c) => {
            c.lock_timeout = Duration::from_secs(cli.lock_timeout);
            c
        }
        Err(e) => std::process::exit(printer.err(name, &e)),
    };

    let result = match &cli.command {
        Command::Push(a) => cmd::push(&ctx, a),
        Command::Pull(a) => cmd::pull(&ctx, a),
        Command::List(a) => cmd::list(&ctx, a),
        Command::Search(a) => cmd::search_cmd(&ctx, a),
        Command::Show(a) => cmd::show(&ctx, a),
        Command::Path { name } => cmd::path(&ctx, name),
        Command::Validate(a) => cmd::validate(&ctx, a),
        Command::Remove(a) => cmd::remove(&ctx, a),
        Command::Agents { cmd } => cmd::agents(&ctx, cmd),
        Command::Config { cmd } => cmd::config(&ctx, cmd),
        Command::Completions { .. } => unreachable!(),
    };
    let code = match result {
        Ok(r) => printer.ok(name, r),
        Err(e) => printer.err(name, &e),
    };
    std::process::exit(code);
}
