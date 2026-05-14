//! `urchin` — the language CLI.
//!
//! First subcommand: `urchin parse <file>` — lex + parse a `.ur` file
//! and pretty-print the resulting AST.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "urchin", version, about = "The Urchin language toolchain")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Parse a `.ur` source file and print the AST.
    Parse {
        /// Path to the source file.
        file: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Parse { file } => match run_parse(&file) {
            Ok(()) => ExitCode::SUCCESS,
            Err(code) => code,
        },
    }
}

fn run_parse(file: &std::path::Path) -> Result<(), ExitCode> {
    let source = std::fs::read_to_string(file).map_err(|e| {
        eprintln!("urchin: cannot read {}: {e}", file.display());
        ExitCode::from(2)
    })?;

    match urchin_parser::parse(&source) {
        Ok(module) => {
            println!("{module:#?}");
            Ok(())
        }
        Err(errors) => {
            for err in &errors {
                eprintln!("{}: {}", file.display(), err.message);
            }
            Err(ExitCode::from(1))
        }
    }
}
