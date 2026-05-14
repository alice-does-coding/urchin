//! Parser — consumes a token stream from `lexer` and produces a `Module` AST.
//!
//! Parsers are generic over the input type so they can be composed without
//! committing to a concrete `Input` shape. The public `parse` adapter
//! constructs the input from the lexer's `Vec<(Token, Span)>`.

use chumsky::input::{Input, ValueInput};
use chumsky::prelude::*;

use crate::ast::{
    BinOp, Expr, Handler, InterfaceMethod, Module, RoleDecl, StateField, Stmt, TypeExpr,
};
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

/// `Handler` ::= `on` TypePath Ident? `{` Stmt* `}`
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
        .then(
            stmt_parser()
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map(|((message_type, binding), body)| Handler {
            message_type,
            binding,
            body,
        })
}

/// `Stmt` ::= `Ident '=' Expr`
///        | `'reply' Expr`
///        | `'broadcast' TypePath ('(' expr_list? ')')?`
///        | `'if' Expr '{' Stmt* '}' ('else' '{' Stmt* '}')?`
///        | `Expr`
///
/// The `Ident '=' …` form covers both local binding and state mutation;
/// the typechecker distinguishes them by whether `Ident` names sealed state.
/// Statements have no separator — adjacent statement-starting tokens delimit them.
fn stmt_parser<'src, I>() -> impl Parser<'src, I, Stmt, extra::Err<Rich<'src, Token, Span>>> + Clone
where
    I: ValueInput<'src, Token = Token, Span = Span>,
{
    recursive(|stmt| {
        let name = select! { Token::Ident(n) => n };
        let segment = select! { Token::Ident(n) => n };

        let assign = name
            .then_ignore(just(Token::Equals))
            .then(expr_parser())
            .map(|(name, value)| Stmt::Assign { name, value });

        let reply = just(Token::KwReply)
            .ignore_then(expr_parser())
            .map(Stmt::Reply);

        let type_path = segment
            .clone()
            .separated_by(just(Token::Dot))
            .at_least(1)
            .collect::<Vec<_>>();

        // Optional arg list: present iff `(` follows the type path.
        let args = expr_parser()
            .separated_by(just(Token::Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(just(Token::LParen), just(Token::RParen))
            .or_not()
            .map(Option::unwrap_or_default);

        let broadcast = just(Token::KwBroadcast)
            .ignore_then(type_path)
            .then(args)
            .map(|(message_type, args)| Stmt::Broadcast { message_type, args });

        let block = stmt
            .clone()
            .repeated()
            .collect::<Vec<_>>()
            .delimited_by(just(Token::LBrace), just(Token::RBrace));

        let if_stmt = just(Token::KwIf)
            .ignore_then(expr_parser())
            .then(block.clone())
            .then(just(Token::KwElse).ignore_then(block).or_not())
            .map(|((cond, then_body), else_body)| Stmt::If {
                cond,
                then_body,
                else_body,
            });

        // ExprStmt is tried last: keyword-led forms have distinguishing leading
        // tokens, so this only catches expressions that start otherwise.
        let expr_stmt = expr_parser().map(Stmt::ExprStmt);

        choice((if_stmt, broadcast, reply, assign, expr_stmt))
    })
}

/// Expression grammar with the precedence ladder
///
///   `~>`             (right-assoc, lowest)
///   `|>`             (left-assoc)
///   `<` `>` `==`     (left-assoc, comparison)
///   `+`              (left-assoc)
///   call / atom      (highest)
///
/// `~>` is right-associative because `a ~> b ~> c` semantically chains
/// state-shift transformations; left-assoc would imply collapsing earlier.
fn expr_parser<'src, I>() -> impl Parser<'src, I, Expr, extra::Err<Rich<'src, Token, Span>>> + Clone
where
    I: ValueInput<'src, Token = Token, Span = Span>,
{
    recursive(|expr| {
        let int = select! { Token::IntLit(n) => Expr::Int(n) };
        let ident = select! { Token::Ident(n) => Expr::Ident(n) };
        let name = select! { Token::Ident(n) => n };

        let parens = expr
            .clone()
            .delimited_by(just(Token::LParen), just(Token::RParen));

        // Function call: `name(arg, arg, ...)`. The leading `name` would
        // otherwise be parsed as a bare ident; the `LParen` after it
        // distinguishes a call.
        let call = name
            .then(
                expr.clone()
                    .separated_by(just(Token::Comma))
                    .allow_trailing()
                    .collect::<Vec<_>>()
                    .delimited_by(just(Token::LParen), just(Token::RParen)),
            )
            .map(|(callee, args)| Expr::Call { callee, args });

        // Try `call` before bare `ident` — both start with `Ident`,
        // and chumsky picks the first-matching branch.
        let atom = choice((int, call, ident, parens));

        // `+` (left-assoc, tightest binary op).
        let add = atom.clone().foldl(
            just(Token::Plus).ignore_then(atom).repeated(),
            |lhs, rhs| Expr::Binary(BinOp::Add, Box::new(lhs), Box::new(rhs)),
        );

        // Comparison (`<`, `>`, `==`) — left-assoc, between `+` and `|>`.
        let cmp_op = choice((
            just(Token::Lt).to(BinOp::Lt),
            just(Token::Gt).to(BinOp::Gt),
            just(Token::EqEq).to(BinOp::Eq),
        ));
        let cmp = add.clone().foldl(
            cmp_op.then(add).repeated(),
            |lhs, (op, rhs)| Expr::Binary(op, Box::new(lhs), Box::new(rhs)),
        );

        // `|>` (left-assoc, between comparisons and `~>`).
        let pipe = cmp.clone().foldl(
            just(Token::Pipe).ignore_then(cmp).repeated(),
            |lhs, rhs| Expr::Binary(BinOp::Pipe, Box::new(lhs), Box::new(rhs)),
        );

        // `~>` (right-assoc, lowest). A right fold via `.then(... .or_not())`
        // recursing into `expr`.
        pipe.clone()
            .then(just(Token::TildeArrow).ignore_then(expr).or_not())
            .map(|(lhs, rhs)| match rhs {
                Some(rhs) => Expr::Binary(BinOp::StateShift, Box::new(lhs), Box::new(rhs)),
                None => lhs,
            })
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

    fn handler_body(src: &str) -> Vec<Stmt> {
        first_role(src)
            .handlers
            .into_iter()
            .next()
            .expect("handler")
            .body
    }

    // --- Existing tests (role / interface / state / handler header) ---

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

    #[test]
    fn parses_function_type_in_state_field() {
        let r = first_role("role X { ~ f: Cue -> Trace }");
        assert!(matches!(r.state[0].ty, TypeExpr::Function(..)));
    }

    #[test]
    fn parses_interface_method() {
        let r = first_role("role AssociativeMemory { recall: Cue -> Trace }");
        assert_eq!(r.interface.len(), 1);
    }

    #[test]
    fn parses_interface_then_state_in_order() {
        let src = "role X { recall: Cue -> Trace ~ traces: int }";
        let r = first_role(src);
        assert_eq!(r.interface.len(), 1);
        assert_eq!(r.state.len(), 1);
    }

    #[test]
    fn state_before_interface_is_a_parse_error() {
        let src = "role X { ~ level: int recall: Cue -> Trace }";
        assert!(parse(src).is_err());
    }

    #[test]
    fn parses_handler_with_type_only() {
        let r = first_role("role X { on Tick {} }");
        assert_eq!(r.handlers.len(), 1);
        assert_eq!(r.handlers[0].binding, None);
    }

    #[test]
    fn parses_handler_with_binding() {
        let r = first_role("role X { on Cue c {} }");
        assert_eq!(r.handlers[0].binding, Some("c".to_string()));
    }

    #[test]
    fn parses_handler_with_dotted_message_type() {
        let r = first_role("role X { on io.sim.Tick {} }");
        assert_eq!(r.handlers[0].message_type.len(), 3);
    }

    #[test]
    fn parses_role_with_all_three_sections() {
        let src = "role X { recall: Cue -> Trace  ~ level: int  on Tick {} }";
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

    // --- Expression grammar ---

    fn parse_expr_in_handler(src: &str) -> Expr {
        let body = handler_body(&format!("role X {{ on Tick {{ {src} }} }}"));
        match body.into_iter().next().expect("statement") {
            Stmt::ExprStmt(e) => e,
            other => panic!("expected ExprStmt, got {other:?}"),
        }
    }

    #[test]
    fn parses_int_literal() {
        assert_eq!(parse_expr_in_handler("42"), Expr::Int(42));
    }

    #[test]
    fn parses_ident_expr() {
        assert_eq!(parse_expr_in_handler("level"), Expr::Ident("level".into()));
    }

    #[test]
    fn parses_addition() {
        assert_eq!(
            parse_expr_in_handler("level + 1"),
            Expr::Binary(
                BinOp::Add,
                Box::new(Expr::Ident("level".into())),
                Box::new(Expr::Int(1))
            )
        );
    }

    #[test]
    fn parses_addition_left_associative() {
        // 1 + 2 + 3  ==  (1 + 2) + 3
        assert_eq!(
            parse_expr_in_handler("1 + 2 + 3"),
            Expr::Binary(
                BinOp::Add,
                Box::new(Expr::Binary(
                    BinOp::Add,
                    Box::new(Expr::Int(1)),
                    Box::new(Expr::Int(2)),
                )),
                Box::new(Expr::Int(3)),
            )
        );
    }

    #[test]
    fn parses_zero_arg_call() {
        assert_eq!(
            parse_expr_in_handler("now()"),
            Expr::Call { callee: "now".into(), args: vec![] }
        );
    }

    #[test]
    fn parses_call_with_args() {
        assert_eq!(
            parse_expr_in_handler("min(level, 1)"),
            Expr::Call {
                callee: "min".into(),
                args: vec![Expr::Ident("level".into()), Expr::Int(1)]
            }
        );
    }

    #[test]
    fn parses_pipe_left_associative() {
        // a |> b() |> c() == (a |> b()) |> c()
        let e = parse_expr_in_handler("traces |> filter() |> rank()");
        // Outer is the second pipe; left side is the first pipe.
        if let Expr::Binary(BinOp::Pipe, lhs, _) = &e {
            assert!(matches!(**lhs, Expr::Binary(BinOp::Pipe, ..)));
        } else {
            panic!("expected outer Pipe, got {e:?}");
        }
    }

    #[test]
    fn parses_state_shift() {
        let body = handler_body("role X { on Tick { level = level ~> level + 1 } }");
        let stmt = body.into_iter().next().unwrap();
        match stmt {
            Stmt::Assign { name, value } => {
                assert_eq!(name, "level");
                // RHS must be a StateShift binary
                assert!(matches!(
                    value,
                    Expr::Binary(BinOp::StateShift, _, _)
                ));
            }
            other => panic!("expected Assign, got {other:?}"),
        }
    }

    #[test]
    fn parses_reply_statement() {
        let body = handler_body("role X { on Cue c { reply level } }");
        assert_eq!(body.len(), 1);
        assert!(matches!(body[0], Stmt::Reply(_)));
    }

    #[test]
    fn parses_assign_statement() {
        let body = handler_body("role X { on Tick { x = 1 } }");
        match &body[0] {
            Stmt::Assign { name, value } => {
                assert_eq!(name, "x");
                assert_eq!(value, &Expr::Int(1));
            }
            other => panic!("expected Assign, got {other:?}"),
        }
    }

    #[test]
    fn parses_multi_statement_handler_body() {
        let body = handler_body(
            "role X { on Tick { x = 1 y = 2 reply x } }",
        );
        assert_eq!(body.len(), 3);
        assert!(matches!(body[0], Stmt::Assign { .. }));
        assert!(matches!(body[1], Stmt::Assign { .. }));
        assert!(matches!(body[2], Stmt::Reply(_)));
    }

    #[test]
    fn parens_control_grouping() {
        // (1 + 2) + 3 — outer add, lhs is parenthesized add
        let e = parse_expr_in_handler("(1 + 2) + 3");
        if let Expr::Binary(BinOp::Add, lhs, rhs) = &e {
            assert_eq!(**rhs, Expr::Int(3));
            assert!(matches!(**lhs, Expr::Binary(BinOp::Add, ..)));
        } else {
            panic!("expected outer Add, got {e:?}");
        }
    }

    // --- Comparisons ---

    #[test]
    fn parses_greater_than() {
        assert!(matches!(
            parse_expr_in_handler("level > 7"),
            Expr::Binary(BinOp::Gt, _, _)
        ));
    }

    #[test]
    fn parses_less_than() {
        assert!(matches!(
            parse_expr_in_handler("level < 7"),
            Expr::Binary(BinOp::Lt, _, _)
        ));
    }

    #[test]
    fn parses_equality() {
        assert!(matches!(
            parse_expr_in_handler("level == 0"),
            Expr::Binary(BinOp::Eq, _, _)
        ));
    }

    #[test]
    fn comparison_binds_looser_than_addition() {
        // `level + 1 > 7` parses as `(level + 1) > 7`
        let e = parse_expr_in_handler("level + 1 > 7");
        if let Expr::Binary(BinOp::Gt, lhs, _) = &e {
            assert!(matches!(**lhs, Expr::Binary(BinOp::Add, ..)));
        } else {
            panic!("expected outer Gt, got {e:?}");
        }
    }

    // --- Conditionals ---

    #[test]
    fn parses_if_without_else() {
        let body = handler_body("role X { on Tick { if level > 7 { reply level } } }");
        match &body[0] {
            Stmt::If { else_body, then_body, .. } => {
                assert!(else_body.is_none());
                assert_eq!(then_body.len(), 1);
            }
            other => panic!("expected If, got {other:?}"),
        }
    }

    #[test]
    fn parses_if_else() {
        let body = handler_body(
            "role X { on Tick { if level > 7 { reply 1 } else { reply 0 } } }",
        );
        match &body[0] {
            Stmt::If { else_body: Some(eb), .. } => {
                assert_eq!(eb.len(), 1);
            }
            other => panic!("expected If with else, got {other:?}"),
        }
    }

    #[test]
    fn parses_nested_if() {
        let body = handler_body(
            "role X { on Tick { if level > 7 { if level > 10 { reply 1 } } } }",
        );
        if let Stmt::If { then_body, .. } = &body[0] {
            assert!(matches!(then_body[0], Stmt::If { .. }));
        } else {
            panic!("expected outer If");
        }
    }

    // --- Broadcast ---

    #[test]
    fn parses_broadcast_no_args() {
        let body = handler_body("role X { on Tick { broadcast Wants } }");
        match &body[0] {
            Stmt::Broadcast { message_type, args } => {
                assert_eq!(message_type, &vec!["Wants".to_string()]);
                assert!(args.is_empty());
            }
            other => panic!("expected Broadcast, got {other:?}"),
        }
    }

    #[test]
    fn parses_broadcast_with_args() {
        let body = handler_body("role X { on Tick { broadcast Found(1) } }");
        match &body[0] {
            Stmt::Broadcast { message_type, args } => {
                assert_eq!(message_type, &vec!["Found".to_string()]);
                assert_eq!(args.len(), 1);
                assert_eq!(args[0], Expr::Int(1));
            }
            other => panic!("expected Broadcast, got {other:?}"),
        }
    }

    #[test]
    fn parses_broadcast_with_dotted_message_type() {
        let body = handler_body("role X { on Tick { broadcast io.sim.Wakeup } }");
        match &body[0] {
            Stmt::Broadcast { message_type, .. } => {
                assert_eq!(message_type, &vec!["io".to_string(), "sim".to_string(), "Wakeup".to_string()]);
            }
            other => panic!("expected Broadcast, got {other:?}"),
        }
    }

    // --- The canonical reactive shape ---

    #[test]
    fn parses_canonical_hunger_handler() {
        let body = handler_body(
            "role Hunger {
               ~ level: int

               on Tick {
                 level = level ~> level + 1
                 if level > 7 {
                   broadcast Wants
                 }
               }
             }",
        );
        assert_eq!(body.len(), 2);
        assert!(matches!(body[0], Stmt::Assign { .. }));
        match &body[1] {
            Stmt::If { then_body, .. } => {
                assert!(matches!(then_body[0], Stmt::Broadcast { .. }));
            }
            other => panic!("expected If with Broadcast inside, got {other:?}"),
        }
    }
}
