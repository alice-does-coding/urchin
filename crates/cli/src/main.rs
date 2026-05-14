//! `urchin` — the language CLI.
//!
//! First subcommand: `urchin parse <file>` — lex + parse a `.ur` file
//! and pretty-print the resulting AST. Errors render through ariadne
//! with source-pointing labels.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ariadne::{Color, Label, Report, ReportKind, Source};
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
    /// Parse and pretty-print a `.ur` source file in canonical form.
    Format {
        /// Path to the source file.
        file: PathBuf,
    },
    /// Parse a `.ur` source file and run semantic checks.
    Check {
        /// Path to the source file.
        file: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Cmd::Parse { file } => run_parse(&file),
        Cmd::Format { file } => run_format(&file),
        Cmd::Check { file } => run_check(&file),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

fn run_parse(file: &Path) -> Result<(), ExitCode> {
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
            let source_id = file.display().to_string();
            for err in errors {
                render_error(&source_id, &source, &err);
            }
            Err(ExitCode::from(1))
        }
    }
}

fn run_format(file: &Path) -> Result<(), ExitCode> {
    let source = std::fs::read_to_string(file).map_err(|e| {
        eprintln!("urchin: cannot read {}: {e}", file.display());
        ExitCode::from(2)
    })?;

    match urchin_parser::parse(&source) {
        Ok(module) => {
            print!("{}", urchin_parser::format(&module));
            Ok(())
        }
        Err(errors) => {
            let source_id = file.display().to_string();
            for err in errors {
                render_error(&source_id, &source, &err);
            }
            Err(ExitCode::from(1))
        }
    }
}

fn run_check(file: &Path) -> Result<(), ExitCode> {
    let source = std::fs::read_to_string(file).map_err(|e| {
        eprintln!("urchin: cannot read {}: {e}", file.display());
        ExitCode::from(2)
    })?;
    let source_id = file.display().to_string();

    let module = match urchin_parser::parse(&source) {
        Ok(m) => m,
        Err(errors) => {
            for err in errors {
                render_error(&source_id, &source, &err);
            }
            return Err(ExitCode::from(1));
        }
    };

    match urchin_types::check(&module) {
        Ok(()) => {
            println!("urchin: {} — ok", file.display());
            Ok(())
        }
        Err(errors) => {
            for err in errors {
                render_check_error(&source_id, &source, &err);
            }
            Err(ExitCode::from(1))
        }
    }
}

/// Render one `ParseError` as an ariadne diagnostic written to stderr.
fn render_error(source_id: &str, source: &str, err: &urchin_parser::ParseError) {
    let span = (source_id.to_string(), err.span.clone());
    Report::build(ReportKind::Error, span.clone())
        .with_message(&err.message)
        .with_label(
            Label::new(span)
                .with_message(&err.message)
                .with_color(Color::Red),
        )
        .finish()
        .eprint((source_id.to_string(), Source::from(source)))
        .expect("write diagnostic to stderr");
}

fn render_check_error(source_id: &str, source: &str, err: &urchin_types::CheckError) {
    let span = (source_id.to_string(), err.span.clone());
    Report::build(ReportKind::Error, span.clone())
        .with_message(&err.message)
        .with_label(
            Label::new(span)
                .with_message(&err.message)
                .with_color(Color::Red),
        )
        .finish()
        .eprint((source_id.to_string(), Source::from(source)))
        .expect("write diagnostic to stderr");
}
