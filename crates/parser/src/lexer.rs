//! Lexer — turns source text into a stream of `(Token, Span)`.
//!
//! Uses chumsky character-level combinators. Comments (`///` … to end of line)
//! and whitespace are skipped; everything else becomes a token.

use chumsky::prelude::*;
use chumsky::span::SimpleSpan;

/// Tokens for the current Urchin subset. Grows as the grammar grows.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Token {
    /// `role`
    KwRole,
    /// `actor`
    KwActor,
    /// `parallel` — dispatch mode
    KwParallel,
    /// `sequence` — dispatch mode (followed by `(A -> B -> C)`)
    KwSequence,
    /// `async` — dispatch mode
    KwAsync,
    /// `on` — handler header (in roles) and dispatch decl (in actors)
    KwOn,
    /// `reply` — reply statement
    KwReply,
    /// `if`
    KwIf,
    /// `else`
    KwElse,
    /// `broadcast`
    KwBroadcast,
    /// PascalCase or snake_case identifier.
    Ident(String),
    /// Integer literal.
    IntLit(i64),
    /// `~`
    Tilde,
    /// `:`
    Colon,
    /// `=` — binding
    Equals,
    /// `==` — equality comparison
    EqEq,
    /// `<` — less-than
    Lt,
    /// `>` — greater-than
    Gt,
    /// `+` — addition (the only arithmetic op for now)
    Plus,
    /// `,`
    Comma,
    /// `->` — function-type / value-flow arrow
    Arrow,
    /// `~>` — state shift (the journal hook)
    TildeArrow,
    /// `|>` — pipe (the lightsaber)
    Pipe,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `[` — list types and list literals
    LBracket,
    /// `]`
    RBracket,
    /// `.` for module paths
    Dot,
}

pub type Span = SimpleSpan<usize>;
pub type Spanned<T> = (T, Span);

/// Lex `source` into a stream of `(Token, Span)` pairs.
///
/// Returns `Err` with chumsky's rich diagnostics if lexing fails.
pub fn lex(source: &str) -> Result<Vec<Spanned<Token>>, Vec<Rich<'_, char>>> {
    lexer().parse(source).into_result()
}

fn lexer<'src>() -> impl Parser<'src, &'src str, Vec<Spanned<Token>>, extra::Err<Rich<'src, char>>>
{
    // `///` to end-of-line (handles both single-line and the multi-line
    // form `///\n…\n///` once we add it; for now just skip-to-newline).
    let line_comment = just("///")
        .then(any().and_is(just('\n').not()).repeated())
        .padded();

    let ident = text::ident().map(|s: &str| match s {
        "role" => Token::KwRole,
        "actor" => Token::KwActor,
        "on" => Token::KwOn,
        "parallel" => Token::KwParallel,
        "sequence" => Token::KwSequence,
        "async" => Token::KwAsync,
        "reply" => Token::KwReply,
        "if" => Token::KwIf,
        "else" => Token::KwElse,
        "broadcast" => Token::KwBroadcast,
        other => Token::Ident(other.to_string()),
    });

    let int = text::int(10)
        .to_slice()
        .map(|s: &str| Token::IntLit(s.parse().expect("lexed digits parse as i64")));

    // Multi-char operators must beat their single-char prefixes — `~>` before
    // `~`, `|>` before any future `|`, `->` before any future `-`, `==` before `=`.
    let punct = choice((
        just("->").to(Token::Arrow),
        just("~>").to(Token::TildeArrow),
        just("|>").to(Token::Pipe),
        just("==").to(Token::EqEq),
        just('~').to(Token::Tilde),
        just(':').to(Token::Colon),
        just('=').to(Token::Equals),
        just('<').to(Token::Lt),
        just('>').to(Token::Gt),
        just('+').to(Token::Plus),
        just(',').to(Token::Comma),
        just('{').to(Token::LBrace),
        just('}').to(Token::RBrace),
        just('(').to(Token::LParen),
        just(')').to(Token::RParen),
        just('[').to(Token::LBracket),
        just(']').to(Token::RBracket),
        just('.').to(Token::Dot),
    ));

    let token = choice((int, ident, punct));

    token
        .map_with(|tok, e| (tok, e.span()))
        .padded_by(line_comment.repeated())
        .padded()
        .repeated()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(src: &str) -> Vec<Token> {
        lex(src).expect("lex error").into_iter().map(|(t, _)| t).collect()
    }

    #[test]
    fn lexes_role_keyword() {
        assert_eq!(toks("role"), vec![Token::KwRole]);
    }

    #[test]
    fn lexes_pascal_ident() {
        assert_eq!(toks("Hunger"), vec![Token::Ident("Hunger".into())]);
    }

    #[test]
    fn lexes_snake_ident() {
        assert_eq!(toks("level"), vec![Token::Ident("level".into())]);
    }

    #[test]
    fn lexes_braces() {
        assert_eq!(toks("{}"), vec![Token::LBrace, Token::RBrace]);
    }

    #[test]
    fn lexes_role_with_state_field() {
        assert_eq!(
            toks("role Hunger { ~ level: int }"),
            vec![
                Token::KwRole,
                Token::Ident("Hunger".into()),
                Token::LBrace,
                Token::Tilde,
                Token::Ident("level".into()),
                Token::Colon,
                Token::Ident("int".into()),
                Token::RBrace,
            ]
        );
    }

    #[test]
    fn skips_line_comments() {
        assert_eq!(
            toks("/// the urchin's smallest role\nrole Hunger {}"),
            vec![
                Token::KwRole,
                Token::Ident("Hunger".into()),
                Token::LBrace,
                Token::RBrace,
            ]
        );
    }

    #[test]
    fn lexes_dotted_path() {
        assert_eq!(
            toks("Memory.Associative"),
            vec![
                Token::Ident("Memory".into()),
                Token::Dot,
                Token::Ident("Associative".into()),
            ]
        );
    }

    #[test]
    fn lexes_arrow() {
        assert_eq!(toks("->"), vec![Token::Arrow]);
    }

    #[test]
    fn lexes_function_type_signature() {
        assert_eq!(
            toks("recall: Cue -> Trace"),
            vec![
                Token::Ident("recall".into()),
                Token::Colon,
                Token::Ident("Cue".into()),
                Token::Arrow,
                Token::Ident("Trace".into()),
            ]
        );
    }

    #[test]
    fn lexes_on_keyword() {
        assert_eq!(toks("on"), vec![Token::KwOn]);
    }

    #[test]
    fn lexes_handler_header() {
        assert_eq!(
            toks("on Tick {}"),
            vec![
                Token::KwOn,
                Token::Ident("Tick".into()),
                Token::LBrace,
                Token::RBrace,
            ]
        );
    }

    #[test]
    fn lexes_int_literal() {
        assert_eq!(toks("42"), vec![Token::IntLit(42)]);
    }

    #[test]
    fn lexes_state_shift_greedily_over_tilde() {
        // `~>` must beat `~` — otherwise we'd lex Tilde then a stray `>`.
        assert_eq!(toks("~>"), vec![Token::TildeArrow]);
    }

    #[test]
    fn lexes_pipe() {
        assert_eq!(toks("|>"), vec![Token::Pipe]);
    }

    #[test]
    fn lexes_reply_keyword() {
        assert_eq!(toks("reply"), vec![Token::KwReply]);
    }

    #[test]
    fn lexes_state_mutation_statement() {
        assert_eq!(
            toks("level = level ~> level + 1"),
            vec![
                Token::Ident("level".into()),
                Token::Equals,
                Token::Ident("level".into()),
                Token::TildeArrow,
                Token::Ident("level".into()),
                Token::Plus,
                Token::IntLit(1),
            ]
        );
    }

    #[test]
    fn lexes_function_call() {
        assert_eq!(
            toks("filter(traces, c)"),
            vec![
                Token::Ident("filter".into()),
                Token::LParen,
                Token::Ident("traces".into()),
                Token::Comma,
                Token::Ident("c".into()),
                Token::RParen,
            ]
        );
    }

    #[test]
    fn lexes_comparison_operators() {
        assert_eq!(toks("< > =="), vec![Token::Lt, Token::Gt, Token::EqEq]);
    }

    #[test]
    fn lexes_eqeq_greedily_over_equals() {
        // `==` must beat `=` `=` — otherwise comparison would parse as
        // assignment-of-an-assignment, which is nonsense.
        assert_eq!(toks("=="), vec![Token::EqEq]);
        assert_eq!(toks("="), vec![Token::Equals]);
    }

    #[test]
    fn lexes_if_else_keywords() {
        assert_eq!(toks("if else"), vec![Token::KwIf, Token::KwElse]);
    }

    #[test]
    fn lexes_broadcast_keyword() {
        assert_eq!(toks("broadcast"), vec![Token::KwBroadcast]);
    }

    #[test]
    fn lexes_conditional_with_broadcast() {
        assert_eq!(
            toks("if level > 7 { broadcast Wants }"),
            vec![
                Token::KwIf,
                Token::Ident("level".into()),
                Token::Gt,
                Token::IntLit(7),
                Token::LBrace,
                Token::KwBroadcast,
                Token::Ident("Wants".into()),
                Token::RBrace,
            ]
        );
    }

    #[test]
    fn lexes_brackets() {
        assert_eq!(toks("[]"), vec![Token::LBracket, Token::RBracket]);
    }

    #[test]
    fn lexes_list_type() {
        assert_eq!(
            toks("[Episode]"),
            vec![
                Token::LBracket,
                Token::Ident("Episode".into()),
                Token::RBracket,
            ]
        );
    }

    #[test]
    fn lexes_actor_keyword() {
        assert_eq!(toks("actor"), vec![Token::KwActor]);
    }

    #[test]
    fn lexes_dispatch_modes() {
        assert_eq!(
            toks("parallel sequence async"),
            vec![Token::KwParallel, Token::KwSequence, Token::KwAsync]
        );
    }
}
