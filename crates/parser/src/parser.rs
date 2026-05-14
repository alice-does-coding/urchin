//! Parser — consumes a token stream from `lexer` and produces a `Module` AST.
//!
//! Parsers are generic over the input type so they can be composed without
//! committing to a concrete `Input` shape. The public `parse` adapter
//! constructs the input from the lexer's `Vec<(Token, Span)>`.

use chumsky::input::{Input, ValueInput};
use chumsky::prelude::*;

use crate::ast::{
    ActorDecl, BinOp, CallArg, DispatchDecl, DispatchMode, Expr, Handler, InterfaceMethod, IoSpine,
    MatchArm, Module, Pattern, RoleDecl, RoleInstance, RoleWire, SpineEvent, StateField, Stmt,
    TypeExpr,
};
use crate::lexer::{lex, Span, Token};
use crate::ParseError;

/// Public entry point. Lex then parse `source`; flatten errors from both
/// stages into a single `Vec<ParseError>`. Error messages use chumsky's
/// `Display` (not `Debug`) so they read cleanly when rendered.
pub fn parse(source: &str) -> Result<Module, Vec<ParseError>> {
    let tokens = lex(source).map_err(|errs| {
        errs.into_iter()
            .map(|e| ParseError {
                message: e.to_string(),
                span: e.span().into_range(),
            })
            .collect::<Vec<_>>()
    })?;

    let eoi: Span = (source.len()..source.len()).into();
    let input = tokens.as_slice().map(eoi, |(t, s)| (t, s));

    module_parser().parse(input).into_result().map_err(|errs| {
        errs.into_iter()
            .map(|e| ParseError {
                message: e.to_string(),
                span: e.span().into_range(),
            })
            .collect()
    })
}

/// `Module` ::= (RoleDecl | ActorDecl)*
///
/// Top-level declarations in any order. Either kind sorts into its own
/// vector on the `Module`.
fn module_parser<'src, I>(
) -> impl Parser<'src, I, Module, extra::Err<Rich<'src, Token, Span>>>
where
    I: ValueInput<'src, Token = Token, Span = Span>,
{
    enum TopLevel {
        Role(RoleDecl),
        Actor(ActorDecl),
    }

    let item = choice((
        role_decl_parser().map(TopLevel::Role),
        actor_decl_parser().map(TopLevel::Actor),
    ));

    item.repeated().collect::<Vec<_>>().map(|items| {
        let mut module = Module::default();
        for item in items {
            match item {
                TopLevel::Role(r) => module.roles.push(r),
                TopLevel::Actor(a) => module.actors.push(a),
            }
        }
        module
    })
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
/// `Stmt` and `Expr` are mutually recursive — `Stmt` contains expressions
/// (RHS of assigns, broadcast args, etc.), and `Expr::Match` contains
/// statement-block arm bodies. Both parsers are built together via nested
/// `recursive` so the cycle resolves at parse-time, not at parser-build-time.
/// (Calling `expr_parser()` from inside a `stmt_parser()` definition would
/// rebuild the parser tree on every call and stack-overflow.)
fn stmt_parser<'src, I>() -> impl Parser<'src, I, Stmt, extra::Err<Rich<'src, Token, Span>>> + Clone
where
    I: ValueInput<'src, Token = Token, Span = Span>,
{
    recursive(|stmt| {
        let expr = expr_parser_with_stmt(stmt.clone());

        let name = select! { Token::Ident(n) => n };
        let segment = select! { Token::Ident(n) => n };

        let assign = name
            .then_ignore(just(Token::Equals))
            .then(expr.clone())
            .map(|(name, value)| Stmt::Assign { name, value });

        let reply = just(Token::KwReply)
            .ignore_then(expr.clone())
            .map(Stmt::Reply);

        let type_path = segment
            .clone()
            .separated_by(just(Token::Dot))
            .at_least(1)
            .collect::<Vec<_>>();

        let args = expr
            .clone()
            .separated_by(just(Token::Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(just(Token::LParen), just(Token::RParen))
            .or_not()
            .map(Option::unwrap_or_default);

        let broadcast = just(Token::KwBroadcast)
            .ignore_then(type_path)
            .then(args)
            .map_with(|(message_type, args), e| {
                let s: Span = e.span();
                Stmt::Broadcast {
                    message_type,
                    args,
                    span: s.start..s.end,
                }
            });

        let block = stmt
            .clone()
            .repeated()
            .collect::<Vec<_>>()
            .delimited_by(just(Token::LBrace), just(Token::RBrace));

        let if_stmt = just(Token::KwIf)
            .ignore_then(expr.clone())
            .then(block.clone())
            .then(just(Token::KwElse).ignore_then(block).or_not())
            .map(|((cond, then_body), else_body)| Stmt::If {
                cond,
                then_body,
                else_body,
            });

        let expr_stmt = expr.map(Stmt::ExprStmt);

        choice((if_stmt, broadcast, reply, assign, expr_stmt))
    })
}

/// Expression grammar with the precedence ladder
///
///   `~>`             (right-assoc, lowest)
///   `|>`             (left-assoc)
///   `<` `>` `==`     (left-assoc, comparison)
///   `+` `-`          (left-assoc, additive)
///   `*` `/`          (left-assoc, multiplicative)
///   `.field` access / call / atom (highest)
///
/// `~>` is right-associative because `a ~> b ~> c` semantically chains
/// state-shift transformations; left-assoc would imply collapsing earlier.
///
/// Takes a `stmt` parameter so the match-arm-block recursion can resolve
/// — see `stmt_parser` for the pairing.
fn expr_parser_with_stmt<'src, I, S>(
    stmt: S,
) -> impl Parser<'src, I, Expr, extra::Err<Rich<'src, Token, Span>>> + Clone
where
    I: ValueInput<'src, Token = Token, Span = Span>,
    S: Parser<'src, I, Stmt, extra::Err<Rich<'src, Token, Span>>> + Clone + 'src,
{
    recursive(|expr| {
        let int = select! { Token::IntLit(n) => Expr::Int(n) };
        let float = select! { Token::FloatLit(n) => Expr::Float(n) };
        let string = select! { Token::StrLit(s) => Expr::Str(s) };
        let ident = select! { Token::Ident(n) => Expr::Ident(n) };
        let name = select! { Token::Ident(n) => n };

        let parens = expr
            .clone()
            .delimited_by(just(Token::LParen), just(Token::RParen));

        // Call arg: `name: expr` (named) is tried before bare `expr` (positional).
        // The named form is unambiguous because `Ident Colon` only appears here
        // — type annotations live in declaration positions, not expressions.
        let arg = choice((
            name.then_ignore(just(Token::Colon))
                .then(expr.clone())
                .map(|(name, value)| CallArg::Named { name, value }),
            expr.clone().map(CallArg::Positional),
        ));

        // Function call: `name(arg, arg, ...)`. The leading `name` would
        // otherwise be parsed as a bare ident; the `LParen` after it
        // distinguishes a call.
        let call = name
            .then(
                arg.separated_by(just(Token::Comma))
                    .allow_trailing()
                    .collect::<Vec<_>>()
                    .delimited_by(just(Token::LParen), just(Token::RParen)),
            )
            .map(|(callee, args)| Expr::Call { callee, args });

        // List literal: `[a, b, c]`. Empty `[]` is allowed.
        let list = expr
            .clone()
            .separated_by(just(Token::Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(just(Token::LBracket), just(Token::RBracket))
            .map(Expr::List);

        // Pattern: `_` (wildcard) or a dotted path (constructor).
        let pat_segment = select! { Token::Ident(n) => n };
        let pattern = pat_segment
            .clone()
            .separated_by(just(Token::Dot))
            .at_least(1)
            .collect::<Vec<_>>()
            .map(|segs| {
                if segs.len() == 1 && segs[0] == "_" {
                    Pattern::Wildcard
                } else {
                    Pattern::Constructor(segs)
                }
            });

        // Match arm body: either a `{ stmt* }` block or a single statement
        // (which can itself be a bare expression via Stmt::ExprStmt). Both
        // lower to a `Vec<Stmt>` for AST uniformity. Using `stmt` here
        // (not `expr`) means `Calm -> broadcast Hum` parses — broadcast is
        // a statement-form, not an expression.
        let arm_block = stmt
            .clone()
            .repeated()
            .collect::<Vec<_>>()
            .delimited_by(just(Token::LBrace), just(Token::RBrace));
        let arm_single = stmt.clone().map(|s| vec![s]);
        let arm_body = choice((arm_block, arm_single));

        let match_arm = pattern
            .then_ignore(just(Token::Arrow))
            .then(arm_body)
            .map(|(pattern, body)| MatchArm { pattern, body });

        // `match scrutinee { arm* }` — arms are whitespace-separated; the
        // expression parser stops at non-operator tokens, so the next arm's
        // pattern naturally ends the previous arm's body.
        let match_expr = just(Token::KwMatch)
            .ignore_then(expr.clone())
            .then(
                match_arm
                    .repeated()
                    .collect::<Vec<_>>()
                    .delimited_by(just(Token::LBrace), just(Token::RBrace)),
            )
            .map(|(scrutinee, arms)| Expr::Match {
                scrutinee: Box::new(scrutinee),
                arms,
            });

        // Try `call` before bare `ident` — both start with `Ident`,
        // and chumsky picks the first-matching branch. `match` is keyword-led
        // so it's unambiguous.
        let primary = choice((match_expr, float, int, string, call, ident, list, parens));

        // Field access: `.field` chained any number of times after a primary.
        // Left-associative: `a.b.c` parses as `(a.b).c`.
        let field = select! { Token::Ident(n) => n };
        let atom = primary.foldl(
            just(Token::Dot).ignore_then(field).repeated(),
            |obj, field| Expr::FieldAccess {
                object: Box::new(obj),
                field,
            },
        );

        // Multiplicative (`*`, `/`) — left-assoc, tightest binary level.
        let mul_op = choice((
            just(Token::Star).to(BinOp::Mul),
            just(Token::Slash).to(BinOp::Div),
        ));
        let mul = atom.clone().foldl(
            mul_op.then(atom).repeated(),
            |lhs, (op, rhs)| Expr::Binary(op, Box::new(lhs), Box::new(rhs)),
        );

        // Additive (`+`, `-`) — left-assoc, between mul and comparison.
        let add_op = choice((
            just(Token::Plus).to(BinOp::Add),
            just(Token::Minus).to(BinOp::Sub),
        ));
        let add = mul.clone().foldl(
            add_op.then(mul).repeated(),
            |lhs, (op, rhs)| Expr::Binary(op, Box::new(lhs), Box::new(rhs)),
        );

        // Comparison (`<`, `>`, `==`) — left-assoc, between `+ -` and `|>`.
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

/// `ActorDecl` ::= `actor` ident `{` IoSpine* RoleInstance* DispatchDecl* `}`
///
/// Body sections are strictly ordered per SPEC.md (§4 sketch / 2026-05-14):
/// IO spines first (the substrate the actor talks to), then role instances
/// with their IO + role-to-role wiring, then dispatch declarations. Each
/// section can be empty. Order is enforced — an IO spine after a role
/// instance is a parse error.
///
/// `IoSpine`      := ident `:` `io.<path>`
/// `RoleInstance` := ident `(` ident_list? `)` ( `(` wire_list? `)` )?
/// `Wire`         := ident `->` ident
/// `DispatchDecl` := `on` ident `.` ident DispatchMode
fn actor_decl_parser<'src, I>(
) -> impl Parser<'src, I, ActorDecl, extra::Err<Rich<'src, Token, Span>>> + Clone
where
    I: ValueInput<'src, Token = Token, Span = Span>,
{
    let segment = select! { Token::Ident(n) => n };
    let actor_name = select! { Token::Ident(n) => n };

    // IO spine: `name : io.<path>`. The colon distinguishes it from a
    // role instance (which uses `(`).
    let io_path = segment
        .clone()
        .separated_by(just(Token::Dot))
        .at_least(1)
        .collect::<Vec<_>>();
    let io_spine = segment
        .clone()
        .then_ignore(just(Token::Colon))
        .then(io_path)
        .map(|(name, io_path)| IoSpine { name, io_path });

    // Role instance: `name(io_args)(wires)?`. The `(` distinguishes it
    // from an IO spine (which uses `:`).
    let ident_list = segment
        .clone()
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LParen), just(Token::RParen));
    let wire = segment
        .clone()
        .then_ignore(just(Token::Arrow))
        .then(segment.clone())
        .map(|(source, method)| RoleWire { source, method });
    let wire_list = wire
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LParen), just(Token::RParen))
        .or_not()
        .map(Option::unwrap_or_default);
    let role_instance = segment
        .clone()
        .then(ident_list)
        .then(wire_list)
        .map(|((name, io_args), wires)| RoleInstance { name, io_args, wires });

    // Dispatch decl: `on spine.event <mode>`. The `on` keyword
    // distinguishes it from a role instance.
    let spine_event = segment
        .clone()
        .then_ignore(just(Token::Dot))
        .then(segment.clone())
        .map(|(spine, event)| SpineEvent { spine, event });
    let dispatch_mode = choice((
        just(Token::KwParallel).to(DispatchMode::Parallel),
        just(Token::KwAsync).to(DispatchMode::Async),
        just(Token::KwSequence)
            .ignore_then(
                segment
                    .clone()
                    .separated_by(just(Token::Arrow))
                    .at_least(1)
                    .collect::<Vec<_>>()
                    .delimited_by(just(Token::LParen), just(Token::RParen)),
            )
            .map(DispatchMode::Sequence),
    ));
    let dispatch = just(Token::KwOn)
        .ignore_then(spine_event)
        .then(dispatch_mode)
        .map(|(event, mode)| DispatchDecl { event, mode });

    // Body in strict section order: spines, then instances, then dispatch.
    let body = io_spine
        .repeated()
        .collect::<Vec<_>>()
        .then(role_instance.repeated().collect::<Vec<_>>())
        .then(dispatch.repeated().collect::<Vec<_>>())
        .delimited_by(just(Token::LBrace), just(Token::RBrace));

    let parent_clause = just(Token::At).ignore_then(segment.clone()).or_not();

    just(Token::KwActor)
        .ignore_then(actor_name)
        .then(parent_clause)
        .then(body)
        .map(|((name, parent), ((io_spines, role_instances), dispatch))| ActorDecl {
            name,
            parent,
            io_spines,
            role_instances,
            dispatch,
        })
}

/// `TypeExpr` ::= TypeAtom (`->` TypeExpr EffectSet?)?     // right-assoc
/// `TypeAtom` ::= `[` TypeExpr `]`
///              | Ident (`.` Ident)*
/// `EffectSet` ::= `/` `{` (effect_path (`,` effect_path)*)? `}`
fn type_expr_parser<'src, I>(
) -> impl Parser<'src, I, TypeExpr, extra::Err<Rich<'src, Token, Span>>> + Clone
where
    I: ValueInput<'src, Token = Token, Span = Span>,
{
    recursive(|type_expr| {
        let segment = select! { Token::Ident(n) => n };
        let dotted = segment
            .clone()
            .separated_by(just(Token::Dot))
            .at_least(1)
            .collect::<Vec<_>>();

        let path = dotted.clone().map(TypeExpr::Path);

        let list = type_expr
            .clone()
            .delimited_by(just(Token::LBracket), just(Token::RBracket))
            .map(|inner| TypeExpr::List(Box::new(inner)));

        let atom = choice((list, path));

        // Optional effect set following an arrow's RHS.
        let effect_set = just(Token::Slash)
            .ignore_then(
                dotted
                    .clone()
                    .separated_by(just(Token::Comma))
                    .allow_trailing()
                    .collect::<Vec<_>>()
                    .delimited_by(just(Token::LBrace), just(Token::RBrace)),
            )
            .or_not()
            .map(Option::unwrap_or_default);

        atom.foldl(
            just(Token::Arrow)
                .ignore_then(type_expr)
                .then(effect_set)
                .repeated()
                .at_most(1),
            |lhs, (rhs, effects)| TypeExpr::Function {
                from: Box::new(lhs),
                to: Box::new(rhs),
                effects,
            },
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
        assert!(matches!(r.state[0].ty, TypeExpr::Function { .. }));
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
    fn parses_call_with_positional_args() {
        assert_eq!(
            parse_expr_in_handler("min(level, 1)"),
            Expr::Call {
                callee: "min".into(),
                args: vec![
                    CallArg::Positional(Expr::Ident("level".into())),
                    CallArg::Positional(Expr::Int(1)),
                ],
            }
        );
    }

    #[test]
    fn parses_call_with_named_arg() {
        assert_eq!(
            parse_expr_in_handler("filter(by: c)"),
            Expr::Call {
                callee: "filter".into(),
                args: vec![CallArg::Named {
                    name: "by".into(),
                    value: Expr::Ident("c".into()),
                }],
            }
        );
    }

    #[test]
    fn parses_call_with_mixed_args() {
        let e = parse_expr_in_handler("rank(traces, by: weight)");
        if let Expr::Call { args, .. } = e {
            assert!(matches!(args[0], CallArg::Positional(_)));
            assert!(matches!(args[1], CallArg::Named { .. }));
        } else {
            panic!("expected Call");
        }
    }

    // --- Field access ---

    #[test]
    fn parses_single_field_access() {
        assert_eq!(
            parse_expr_in_handler("c.weight"),
            Expr::FieldAccess {
                object: Box::new(Expr::Ident("c".into())),
                field: "weight".into(),
            }
        );
    }

    #[test]
    fn parses_chained_field_access_left_associative() {
        // a.b.c == (a.b).c
        let e = parse_expr_in_handler("a.b.c");
        if let Expr::FieldAccess { object, field } = &e {
            assert_eq!(field, "c");
            assert!(matches!(**object, Expr::FieldAccess { .. }));
        } else {
            panic!("expected outer FieldAccess");
        }
    }

    #[test]
    fn parses_field_access_in_named_arg() {
        // The full lightsaber move: rank(by: c.weight)
        let e = parse_expr_in_handler("rank(by: c.weight)");
        if let Expr::Call { args, .. } = e {
            match &args[0] {
                CallArg::Named { name, value } => {
                    assert_eq!(name, "by");
                    assert!(matches!(value, Expr::FieldAccess { .. }));
                }
                other => panic!("expected Named, got {other:?}"),
            }
        } else {
            panic!("expected Call");
        }
    }

    #[test]
    fn parses_full_pipe_chain() {
        // The canonical lightsaber chain
        let e = parse_expr_in_handler("traces |> filter(by: c) |> rank(by: c.weight)");
        // Outer is the second pipe
        if let Expr::Binary(BinOp::Pipe, lhs, rhs) = &e {
            assert!(matches!(**lhs, Expr::Binary(BinOp::Pipe, ..)));
            assert!(matches!(**rhs, Expr::Call { .. }));
        } else {
            panic!("expected outer Pipe, got {e:?}");
        }
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
            Stmt::Broadcast { message_type, args, .. } => {
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
            Stmt::Broadcast { message_type, args, .. } => {
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

    // --- List types ---

    #[test]
    fn parses_list_type_in_state() {
        let r = first_role("role X { ~ episodes: [Episode] }");
        assert_eq!(
            r.state[0].ty,
            TypeExpr::List(Box::new(TypeExpr::Path(vec!["Episode".into()])))
        );
    }

    #[test]
    fn parses_function_returning_list() {
        let r = first_role("role X { recall: Cue -> [Trace] }");
        if let TypeExpr::Function { to, .. } = &r.interface[0].ty {
            assert!(matches!(**to, TypeExpr::List(_)));
        } else {
            panic!("expected Function type");
        }
    }

    // --- Match expressions ---

    #[test]
    fn parses_match_with_two_arms() {
        let body = handler_body(
            "role X { on Stimulus s { match s { Threat -> retreat() Food -> approach() } } }",
        );
        match &body[0] {
            Stmt::ExprStmt(Expr::Match { scrutinee, arms }) => {
                assert!(matches!(**scrutinee, Expr::Ident(_)));
                assert_eq!(arms.len(), 2);
                match &arms[0].pattern {
                    Pattern::Constructor(p) => assert_eq!(p, &vec!["Threat".to_string()]),
                    _ => panic!(),
                }
            }
            other => panic!("expected Match expr stmt, got {other:?}"),
        }
    }

    #[test]
    fn parses_wildcard_pattern() {
        let body = handler_body(
            "role X { on S s { match s { Threat -> retreat() _ -> idle() } } }",
        );
        if let Stmt::ExprStmt(Expr::Match { arms, .. }) = &body[0] {
            assert_eq!(arms[1].pattern, Pattern::Wildcard);
        } else {
            panic!("expected Match");
        }
    }

    #[test]
    fn parses_dotted_constructor_pattern() {
        let body = handler_body(
            "role X { on E e { match e { io.sim.Tick -> tick() } } }",
        );
        if let Stmt::ExprStmt(Expr::Match { arms, .. }) = &body[0] {
            match &arms[0].pattern {
                Pattern::Constructor(p) => assert_eq!(p.len(), 3),
                _ => panic!(),
            }
        } else {
            panic!("expected Match");
        }
    }

    #[test]
    fn parses_block_bodied_arm() {
        let body = handler_body(
            "role X { on S s { match s { Threat -> { broadcast Retreat reply 1 } _ -> 0 } } }",
        );
        if let Stmt::ExprStmt(Expr::Match { arms, .. }) = &body[0] {
            assert_eq!(arms[0].body.len(), 2);
            assert!(matches!(arms[0].body[0], Stmt::Broadcast { .. }));
            assert!(matches!(arms[0].body[1], Stmt::Reply(_)));
            // Wildcard arm has bare-expr body wrapped to one ExprStmt.
            assert_eq!(arms[1].body.len(), 1);
            assert!(matches!(arms[1].body[0], Stmt::ExprStmt(_)));
        } else {
            panic!("expected Match");
        }
    }

    #[test]
    fn match_can_be_assigned_to_a_local() {
        let body = handler_body(
            "role X { on S s { decision = match s { Yes -> 1 No -> 0 } } }",
        );
        match &body[0] {
            Stmt::Assign { name, value: Expr::Match { .. } } => {
                assert_eq!(name, "decision");
            }
            other => panic!("expected Assign with Match value, got {other:?}"),
        }
    }

    // --- Floats, strings, arithmetic ---

    #[test]
    fn parses_float_literal() {
        assert_eq!(parse_expr_in_handler("0.01"), Expr::Float(0.01));
    }

    #[test]
    fn parses_string_literal() {
        assert_eq!(parse_expr_in_handler("\"hello\""), Expr::Str("hello".into()));
    }

    #[test]
    fn parses_subtraction() {
        assert!(matches!(
            parse_expr_in_handler("level - 1"),
            Expr::Binary(BinOp::Sub, _, _)
        ));
    }

    #[test]
    fn parses_multiplication() {
        assert!(matches!(
            parse_expr_in_handler("level * 2"),
            Expr::Binary(BinOp::Mul, _, _)
        ));
    }

    #[test]
    fn parses_division() {
        assert!(matches!(
            parse_expr_in_handler("level / 2"),
            Expr::Binary(BinOp::Div, _, _)
        ));
    }

    #[test]
    fn multiplicative_binds_tighter_than_additive() {
        // `1 + 2 * 3` parses as `1 + (2 * 3)`
        let e = parse_expr_in_handler("1 + 2 * 3");
        if let Expr::Binary(BinOp::Add, _, rhs) = &e {
            assert!(matches!(**rhs, Expr::Binary(BinOp::Mul, ..)));
        } else {
            panic!("expected Add as outer, got {e:?}");
        }
    }

    #[test]
    fn float_arithmetic() {
        let e = parse_expr_in_handler("level + 0.01");
        if let Expr::Binary(BinOp::Add, _, rhs) = &e {
            assert_eq!(**rhs, Expr::Float(0.01));
        } else {
            panic!("expected Add, got {e:?}");
        }
    }

    // --- Effect annotations ---

    #[test]
    fn function_without_effect_clause_has_empty_effects() {
        let r = first_role("role X { recall: Cue -> Trace }");
        if let TypeExpr::Function { effects, .. } = &r.interface[0].ty {
            assert!(effects.is_empty());
        } else {
            panic!("expected Function");
        }
    }

    #[test]
    fn parses_single_effect_annotation() {
        let r = first_role("role X { fetch: Url -> Response / {io.http} }");
        if let TypeExpr::Function { effects, .. } = &r.interface[0].ty {
            assert_eq!(effects, &vec![vec!["io".to_string(), "http".to_string()]]);
        } else {
            panic!("expected Function");
        }
    }

    #[test]
    fn parses_multiple_effects() {
        let r = first_role("role X { tick: Unit -> Unit / {io.sim.clock, io.sim.comms} }");
        if let TypeExpr::Function { effects, .. } = &r.interface[0].ty {
            assert_eq!(effects.len(), 2);
            assert_eq!(effects[0], vec!["io".to_string(), "sim".to_string(), "clock".to_string()]);
            assert_eq!(effects[1], vec!["io".to_string(), "sim".to_string(), "comms".to_string()]);
        } else {
            panic!("expected Function");
        }
    }

    #[test]
    fn parses_empty_effect_set_explicitly() {
        // `T -> U / {}` is allowed; parses as empty effects (same as omitting `/ {}`).
        let r = first_role("role X { f: A -> B / {} }");
        if let TypeExpr::Function { effects, .. } = &r.interface[0].ty {
            assert!(effects.is_empty());
        } else {
            panic!("expected Function");
        }
    }

    #[test]
    fn effect_annotation_works_on_state_field_function_type() {
        let r = first_role("role X { ~ handler: Event -> Unit / {io.sim.comms} }");
        if let TypeExpr::Function { effects, .. } = &r.state[0].ty {
            assert_eq!(effects.len(), 1);
        } else {
            panic!("expected Function");
        }
    }

    #[test]
    fn parses_nested_list_type() {
        let r = first_role("role X { ~ matrix: [[int]] }");
        assert_eq!(
            r.state[0].ty,
            TypeExpr::List(Box::new(TypeExpr::List(Box::new(TypeExpr::Path(vec![
                "int".into()
            ])))))
        );
    }

    // --- List literals ---

    #[test]
    fn parses_empty_list_literal() {
        assert_eq!(parse_expr_in_handler("[]"), Expr::List(vec![]));
    }

    #[test]
    fn parses_list_literal_with_elements() {
        assert_eq!(
            parse_expr_in_handler("[1, 2, 3]"),
            Expr::List(vec![Expr::Int(1), Expr::Int(2), Expr::Int(3)])
        );
    }

    #[test]
    fn parses_list_concat_with_addition() {
        // `episodes + [e]` — typechecker may or may not allow this for lists,
        // but the parser just produces a Binary(Add, ident, list).
        let e = parse_expr_in_handler("episodes + [e]");
        if let Expr::Binary(BinOp::Add, lhs, rhs) = &e {
            assert!(matches!(**lhs, Expr::Ident(_)));
            assert!(matches!(**rhs, Expr::List(_)));
        } else {
            panic!("expected Binary Add, got {e:?}");
        }
    }

    // --- Actor declarations ---

    fn first_actor(src: &str) -> ActorDecl {
        let m = parse(src).expect("parse");
        m.actors.into_iter().next().expect("actor")
    }

    #[test]
    fn parses_empty_actor() {
        let a = first_actor("actor mind {}");
        assert_eq!(a.name, "mind");
        assert!(a.parent.is_none());
        assert!(a.io_spines.is_empty());
        assert!(a.role_instances.is_empty());
        assert!(a.dispatch.is_empty());
    }

    #[test]
    fn parses_actor_with_parent() {
        let a = first_actor("actor mind @ rubberDuck {}");
        assert_eq!(a.name, "mind");
        assert_eq!(a.parent.as_deref(), Some("rubberDuck"));
    }

    #[test]
    fn parses_actor_with_parent_and_body() {
        let a = first_actor(
            "actor mind @ rubberDuck {
               clock: io.sim.clock
               hunger(clock)
               on clock.tick parallel
             }",
        );
        assert_eq!(a.parent.as_deref(), Some("rubberDuck"));
        assert_eq!(a.io_spines.len(), 1);
        assert_eq!(a.role_instances.len(), 1);
        assert_eq!(a.dispatch.len(), 1);
    }

    #[test]
    fn parses_actor_with_io_spines() {
        let a = first_actor(
            "actor mind {
               http: io.http.server
               clock: io.sim.clock
             }",
        );
        assert_eq!(a.io_spines.len(), 2);
        assert_eq!(a.io_spines[0].name, "http");
        assert_eq!(
            a.io_spines[0].io_path,
            vec!["io".to_string(), "http".to_string(), "server".to_string()]
        );
    }

    #[test]
    fn parses_role_instance_with_io_args() {
        let a = first_actor(
            "actor mind {
               clock: io.sim.clock
               hunger(clock)
             }",
        );
        assert_eq!(a.role_instances.len(), 1);
        let inst = &a.role_instances[0];
        assert_eq!(inst.name, "hunger");
        assert_eq!(inst.io_args, vec!["clock".to_string()]);
        assert!(inst.wires.is_empty());
    }

    #[test]
    fn parses_role_instance_with_multiple_io_args() {
        let a = first_actor(
            "actor mind {
               clock: io.sim.clock
               siblings: io.sim.comms.peer
               episodicMemory(clock, siblings)
             }",
        );
        let inst = &a.role_instances[0];
        assert_eq!(inst.io_args, vec!["clock".to_string(), "siblings".to_string()]);
    }

    #[test]
    fn parses_role_instance_with_wires() {
        let a = first_actor(
            "actor mind {
               clock: io.sim.clock
               episodicMemory(clock)
               voice(clock)(episodicMemory -> recall)
             }",
        );
        let voice = &a.role_instances[1];
        assert_eq!(voice.name, "voice");
        assert_eq!(voice.wires.len(), 1);
        assert_eq!(voice.wires[0].source, "episodicMemory");
        assert_eq!(voice.wires[0].method, "recall");
    }

    #[test]
    fn parses_role_instance_with_multiple_wires() {
        let a = first_actor(
            "actor mind {
               clock: io.sim.clock
               mem(clock)
               other(clock)
               voice(clock)(mem -> recall, other -> store)
             }",
        );
        let voice = a.role_instances.last().unwrap();
        assert_eq!(voice.wires.len(), 2);
        assert_eq!(voice.wires[0].source, "mem");
        assert_eq!(voice.wires[1].source, "other");
    }

    #[test]
    fn parses_dispatch_parallel() {
        let a = first_actor(
            "actor mind {
               clock: io.sim.clock
               on clock.tick parallel
             }",
        );
        assert_eq!(a.dispatch.len(), 1);
        assert_eq!(a.dispatch[0].event.spine, "clock");
        assert_eq!(a.dispatch[0].event.event, "tick");
        assert_eq!(a.dispatch[0].mode, DispatchMode::Parallel);
    }

    #[test]
    fn parses_dispatch_async() {
        let a = first_actor(
            "actor mind {
               siblings: io.sim.comms.peer
               on siblings.message async
             }",
        );
        assert_eq!(a.dispatch[0].mode, DispatchMode::Async);
    }

    #[test]
    fn parses_dispatch_sequence_with_lowercase_instances() {
        let a = first_actor(
            "actor mind {
               clock: io.sim.clock
               hunger(clock)
               voice(clock)
               on clock.tick sequence(hunger -> voice)
             }",
        );
        match &a.dispatch[0].mode {
            DispatchMode::Sequence(insts) => {
                assert_eq!(insts, &vec!["hunger".to_string(), "voice".to_string()]);
            }
            other => panic!("expected Sequence, got {other:?}"),
        }
    }

    #[test]
    fn parses_actor_with_all_three_sections() {
        let a = first_actor(
            "actor mind {
               clock:    io.sim.clock
               siblings: io.sim.comms.peer

               episodicMemory(clock, siblings)
               voice(clock)(episodicMemory -> recall)
               hunger(clock)

               on clock.tick sequence(hunger -> voice)
             }",
        );
        assert_eq!(a.io_spines.len(), 2);
        assert_eq!(a.role_instances.len(), 3);
        assert_eq!(a.dispatch.len(), 1);
        assert_eq!(a.role_instances[1].wires[0].source, "episodicMemory");
    }

    #[test]
    fn role_instance_after_dispatch_is_a_parse_error() {
        // Section order: spines, then instances, then dispatch — strict.
        let src = "actor mind {
                     clock: io.sim.clock
                     on clock.tick parallel
                     hunger(clock)
                   }";
        assert!(parse(src).is_err());
    }

    #[test]
    fn io_spine_after_role_instance_is_a_parse_error() {
        let src = "actor mind {
                     hunger(clock)
                     clock: io.sim.clock
                   }";
        assert!(parse(src).is_err());
    }

    #[test]
    fn module_can_hold_roles_and_actors_together() {
        let m = parse(
            "role Hunger { ~ level: int }
             actor mind { clock: io.sim.clock  hunger(clock) }",
        )
        .expect("parse");
        assert_eq!(m.roles.len(), 1);
        assert_eq!(m.actors.len(), 1);
    }

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
