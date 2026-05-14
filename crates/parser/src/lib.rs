//! Urchin parser.
//!
//! Two-stage: lexer produces tokens, parser consumes tokens. Both built
//! with chumsky. The public API is `parse(source) -> Result<Module, Vec<ParseError>>`.

pub mod ast;
pub mod lexer;
pub mod parser;

pub use ast::*;
pub use parser::parse;

/// Parse error — currently a thin wrapper around a message + char range.
/// Will grow into a proper diagnostic type once we wire ariadne or miette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub span: std::ops::Range<usize>,
}
