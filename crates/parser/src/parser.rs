//! Parser — consumes a token stream from `lexer` and produces a `Module` AST.
//!
//! Parsers are generic over the input type so they can be composed without
//! committing to a concrete `Input` shape. The public `parse` adapter
//! constructs the input from the lexer's `Vec<(Token, Span)>`.

use chumsky::input::{Input, ValueInput};
use chumsky::prelude::*;

use crate::ast::{Handler, InterfaceMethod, Module, RoleDecl, StateField, TypeExpr};
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

/// `RoleDecl` ::= `role` Ident `{` InterfaceMethod* StateField* Handler* `}`
///
/// Per SPEC.md §3.1 the three sections appear in that order; each is optional.
/// Order is enforced syntactically — a state field after a handler is a parse
/// error, not a reorder hint.
fn role_decl_parser<'src, I>(
) -> impl Parser<'src, I, RoleDecl, extra::Err<Rich<'src, Token, Span>>> + Clone
where
    I: ValueInput<'src, Token = Token, Span = Span>,
{
    let name = select! { Token::Ident(n) => n };

    let body = interface_method_parser()
        .repeated()
        .collect::<Vec<_>>()
        .then(state_field_parser().repeated().collect::<Vec<_>>())
        .then(handler_parser().repeated().collect::<Vec<_>>())
        .delimited_by(just(Token::LBrace), just(Token::RBrace));

    just(Token::KwRole)
        .ignore_then(name)
        .then(body)
        .map(|(name, ((interface, state), handlers))| RoleDecl {
            name,
            interface,
            state,
            handlers,
        })
}

/// `InterfaceMethod` ::= Ident `:` TypeExpr
///
/// Distinguished from `StateField` only by the absence of a leading `~`.
fn interface_method_parser<'src, I>(
) -> impl Parser<'src, I, InterfaceMethod, extra::Err<Rich<'src, Token, Span>>> + Clone
where
    I: ValueInput<'src, Token = Token, Span = Span>,
{
    let name = select! { Token::Ident(n) => n };

    name.then_ignore(just(Token::Colon))
        .then(type_expr_parser())
        .map(|(name, ty)| InterfaceMethod { name, ty })
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

/// `Handler` ::= `on` TypePath Ident? `{` `}`
///
/// Body is currently always empty; expression grammar lands in a follow-up slice.
fn handler_parser<'src, I>(
) -> impl Parser<'src, I, Handler, extra::Err<Rich<'src, Token, Span>>> + Clone
where
    I: ValueInput<'src, Token = Token, Span = Span>,
{
    let segment = select! { Token::Ident(n) => n };
    let type_path = segment
        .clone()
        .separated_by(just(Token::Dot))
        .at_least(1)
        .collect::<Vec<_>>();
    let binding = segment.or_not();

    just(Token::KwOn)
        .ignore_then(type_path)
        .then(binding)
        .then_ignore(just(Token::LBrace))
        .then_ignore(just(Token::RBrace))
        .map(|(message_type, binding)| Handler {
            message_type,
            binding,
        })
}

/// `TypeExpr` ::= TypeAtom (`->` TypeExpr)?    -- right-associative function type
/// `TypeAtom` ::= Ident (`.` Ident)*
fn type_expr_parser<'src, I>(
) -> impl Parser<'src, I, TypeExpr, extra::Err<Rich<'src, Token, Span>>> + Clone
where
    I: ValueInput<'src, Token = Token, Span = Span>,
{
    recursive(|type_expr| {
        let segment = select! { Token::Ident(n) => n };
        let atom = segment
            .separated_by(just(Token::Dot))
            .at_least(1)
            .collect::<Vec<_>>()
            .map(TypeExpr::Path);

        atom.foldl(
            just(Token::Arrow).ignore_then(type_expr).repeated().at_most(1),
            |lhs, rhs| TypeExpr::Function(Box::new(lhs), Box::new(rhs)),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_role(src: &str) -> RoleDecl {
        let m = parse(src).expect("parse");
        m.roles.into_iter().next().expect("role")
    }

    #[test]
    fn parses_empty_role() {
        let r = first_role("role Hunger {}");
        assert_eq!(r.name, "Hunger");
        assert!(r.interface.is_empty());
        assert!(r.state.is_empty());
        assert!(r.handlers.is_empty());
    }

    #[test]
    fn parses_role_with_one_state_field() {
        let r = first_role("role Hunger { ~ level: int }");
        assert_eq!(r.state.len(), 1);
        assert_eq!(r.state[0].name, "level");
        assert_eq!(r.state[0].ty, TypeExpr::Path(vec!["int".into()]));
    }

    #[test]
    fn parses_role_with_multiple_state_fields() {
        let r = first_role("role X { ~ traces: int ~ count: int }");
        assert_eq!(r.state.len(), 2);
    }

    #[test]
    fn parses_dotted_type_path() {
        let r = first_role("role X { ~ mem: Memory.Associative }");
        assert_eq!(
            r.state[0].ty,
            TypeExpr::Path(vec!["Memory".into(), "Associative".into()])
        );
    }

    #[test]
    fn parses_multiple_roles_in_module() {
        let m = parse("role Hunger {} role Voice {}").expect("parse");
        assert_eq!(m.roles.len(), 2);
    }

    #[test]
    fn skips_leading_and_trailing_comments() {
        let r = first_role("/// preamble\nrole Hunger {}\n/// trailing\n");
        assert_eq!(r.name, "Hunger");
    }

    #[test]
    fn empty_input_parses_to_empty_module() {
        assert!(parse("").expect("parse").roles.is_empty());
    }

    #[test]
    fn unrecognized_top_level_token_is_an_error() {
        assert!(parse("~").is_err());
    }

    // --- Function types ---

    #[test]
    fn parses_function_type_in_state_field() {
        let r = first_role("role X { ~ f: Cue -> Trace }");
        assert_eq!(
            r.state[0].ty,
            TypeExpr::Function(
                Box::new(TypeExpr::Path(vec!["Cue".into()])),
                Box::new(TypeExpr::Path(vec!["Trace".into()])),
            )
        );
    }

    // --- Interface methods ---

    #[test]
    fn parses_interface_method() {
        let r = first_role("role AssociativeMemory { recall: Cue -> Trace }");
        assert_eq!(r.interface.len(), 1);
        assert_eq!(r.interface[0].name, "recall");
        assert_eq!(
            r.interface[0].ty,
            TypeExpr::Function(
                Box::new(TypeExpr::Path(vec!["Cue".into()])),
                Box::new(TypeExpr::Path(vec!["Trace".into()])),
            )
        );
    }

    #[test]
    fn parses_interface_then_state_in_order() {
        let src = "role AssociativeMemory { recall: Cue -> Trace ~ traces: int }";
        let r = first_role(src);
        assert_eq!(r.interface.len(), 1);
        assert_eq!(r.state.len(), 1);
    }

    #[test]
    fn state_before_interface_is_a_parse_error() {
        // Per SPEC §3.1: interface, then state, then handlers — order matters.
        let src = "role X { ~ level: int recall: Cue -> Trace }";
        assert!(parse(src).is_err());
    }

    // --- Handlers ---

    #[test]
    fn parses_handler_with_type_only() {
        let r = first_role("role Hunger { on Tick {} }");
        assert_eq!(r.handlers.len(), 1);
        assert_eq!(r.handlers[0].message_type, vec!["Tick".to_string()]);
        assert_eq!(r.handlers[0].binding, None);
    }

    #[test]
    fn parses_handler_with_binding() {
        let r = first_role("role X { on Cue c {} }");
        assert_eq!(r.handlers[0].message_type, vec!["Cue".to_string()]);
        assert_eq!(r.handlers[0].binding, Some("c".to_string()));
    }

    #[test]
    fn parses_handler_with_dotted_message_type() {
        let r = first_role("role X { on io.sim.Tick {} }");
        assert_eq!(
            r.handlers[0].message_type,
            vec!["io".to_string(), "sim".to_string(), "Tick".to_string()]
        );
    }

    #[test]
    fn parses_role_with_all_three_sections() {
        let src = "role Hunger { recall: Cue -> Trace  ~ level: int  on Tick {} }";
        let r = first_role(src);
        assert_eq!(r.interface.len(), 1);
        assert_eq!(r.state.len(), 1);
        assert_eq!(r.handlers.len(), 1);
    }

    #[test]
    fn parses_multiple_handlers() {
        let src = "role X { on Tick {} on Cue c {} }";
        let r = first_role(src);
        assert_eq!(r.handlers.len(), 2);
    }
}
