//! Parser — consumes a token stream from `lexer` and produces a `Module` AST.
//!
//! Parsers are generic over the input type so they can be composed without
//! committing to a concrete `Input` shape. The public `parse` adapter
//! constructs the input from the lexer's `Vec<(Token, Span)>`.

use chumsky::input::{Input, ValueInput};
use chumsky::prelude::*;

use crate::ast::{Module, RoleDecl, StateField, TypeExpr};
use crate::lexer::{lex, Span, Token};
use crate::ParseError;

/// Public entry point. Lex then parse `source`; flatten errors from both
/// stages into a single `Vec<ParseError>`.
pub fn parse(source: &str) -> Result<Module, Vec<ParseError>> {
    let tokens = lex(source).map_err(|errs| {
        errs.into_iter()
            .map(|e| ParseError {
                message: format!("lex error: {e:?}"),
                span: e.span().into_range(),
            })
            .collect::<Vec<_>>()
    })?;

    let eoi: Span = (source.len()..source.len()).into();
    let input = tokens.as_slice().map(eoi, |(t, s)| (t, s));

    module_parser().parse(input).into_result().map_err(|errs| {
        errs.into_iter()
            .map(|e| ParseError {
                message: format!("parse error: {e:?}"),
                span: e.span().into_range(),
            })
            .collect()
    })
}

/// `Module` ::= `RoleDecl`*
fn module_parser<'src, I>(
) -> impl Parser<'src, I, Module, extra::Err<Rich<'src, Token, Span>>>
where
    I: ValueInput<'src, Token = Token, Span = Span>,
{
    role_decl_parser()
        .repeated()
        .collect::<Vec<_>>()
        .map(|roles| Module { roles })
}

/// `RoleDecl` ::= `role` Ident `{` StateField* `}`
fn role_decl_parser<'src, I>(
) -> impl Parser<'src, I, RoleDecl, extra::Err<Rich<'src, Token, Span>>> + Clone
where
    I: ValueInput<'src, Token = Token, Span = Span>,
{
    let name = select! { Token::Ident(n) => n };

    just(Token::KwRole)
        .ignore_then(name)
        .then(
            state_field_parser()
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map(|(name, state)| RoleDecl { name, state })
}

/// `StateField` ::= `~` Ident `:` TypeExpr
fn state_field_parser<'src, I>(
) -> impl Parser<'src, I, StateField, extra::Err<Rich<'src, Token, Span>>> + Clone
where
    I: ValueInput<'src, Token = Token, Span = Span>,
{
    let name = select! { Token::Ident(n) => n };

    just(Token::Tilde)
        .ignore_then(name)
        .then_ignore(just(Token::Colon))
        .then(type_expr_parser())
        .map(|(name, ty)| StateField { name, ty })
}

/// `TypeExpr` ::= Ident (`.` Ident)*  — only dotted paths for now
fn type_expr_parser<'src, I>(
) -> impl Parser<'src, I, TypeExpr, extra::Err<Rich<'src, Token, Span>>> + Clone
where
    I: ValueInput<'src, Token = Token, Span = Span>,
{
    let segment = select! { Token::Ident(n) => n };

    segment
        .separated_by(just(Token::Dot))
        .at_least(1)
        .collect::<Vec<_>>()
        .map(TypeExpr::Path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_empty_role() {
        let m = parse("role Hunger {}").expect("parse");
        assert_eq!(m.roles.len(), 1);
        assert_eq!(m.roles[0].name, "Hunger");
        assert!(m.roles[0].state.is_empty());
    }

    #[test]
    fn parses_role_with_one_state_field() {
        let m = parse("role Hunger { ~ level: int }").expect("parse");
        assert_eq!(m.roles[0].name, "Hunger");
        assert_eq!(m.roles[0].state.len(), 1);
        assert_eq!(m.roles[0].state[0].name, "level");
        assert_eq!(
            m.roles[0].state[0].ty,
            TypeExpr::Path(vec!["int".into()])
        );
    }

    #[test]
    fn parses_role_with_multiple_state_fields() {
        let src = "role AssociativeMemory { ~ traces: int ~ count: int }";
        let m = parse(src).expect("parse");
        assert_eq!(m.roles[0].state.len(), 2);
        assert_eq!(m.roles[0].state[0].name, "traces");
        assert_eq!(m.roles[0].state[1].name, "count");
    }

    #[test]
    fn parses_dotted_type_path() {
        let src = "role X { ~ mem: Memory.Associative }";
        let m = parse(src).expect("parse");
        assert_eq!(
            m.roles[0].state[0].ty,
            TypeExpr::Path(vec!["Memory".into(), "Associative".into()])
        );
    }

    #[test]
    fn parses_multiple_roles_in_module() {
        let m = parse("role Hunger {} role Voice {}").expect("parse");
        assert_eq!(m.roles.len(), 2);
        assert_eq!(m.roles[0].name, "Hunger");
        assert_eq!(m.roles[1].name, "Voice");
    }

    #[test]
    fn skips_leading_and_trailing_comments() {
        let src = "/// the smallest role\nrole Hunger {}\n/// trailing\n";
        let m = parse(src).expect("parse");
        assert_eq!(m.roles[0].name, "Hunger");
    }

    #[test]
    fn empty_input_parses_to_empty_module() {
        let m = parse("").expect("parse");
        assert!(m.roles.is_empty());
    }

    #[test]
    fn unrecognized_top_level_token_is_an_error() {
        assert!(parse("~").is_err());
    }
}
