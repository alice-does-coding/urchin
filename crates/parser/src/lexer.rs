//! Lexer — turns source text into a stream of `(Token, Span)`.
//!
//! Uses chumsky character-level combinators. Comments (`///` … to end of line)
//! and whitespace are skipped; everything else becomes a token.

use std::fmt;

use chumsky::prelude::*;
use chumsky::span::SimpleSpan;

/// Tokens for the current Urchin subset. Grows as the grammar grows.
///
/// Only `PartialEq` is derived — `f64` in `FloatLit` rules out `Eq`/`Hash`,
/// which is fine since chumsky's `just(...)` only needs `PartialEq`.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// `facet`
    KwFacet,
    /// `scheme`
    KwScheme,
    /// `parallel` — dispatch mode
    KwParallel,
    /// `sequence` — dispatch mode (followed by `(A -> B -> C)`)
    KwSequence,
    /// `async` — dispatch mode
    KwAsync,
    /// `on` — handler header (in facets) and dispatch decl (in schemes)
    KwOn,
    /// `if`
    KwIf,
    /// `else`
    KwElse,
    /// `match`
    KwMatch,
    /// PascalCase or snake_case identifier.
    Ident(String),
    /// Integer literal.
    IntLit(i64),
    /// Float literal — `0.01`, `1.0`, `42.5`. Always has a decimal point.
    FloatLit(f64),
    /// String literal — `"hello"`, with `\\` `\"` `\n` `\t` `\r` escapes.
    StrLit(String),
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
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
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
    /// `/` — effect-set separator (`T -> U / {io.http}`).
    /// `///` (the comment marker) is matched first by the lexer's outer
    /// padding pass, so a bare `/` only ever lands here.
    Slash,
    /// `.` for module paths
    Dot,
    /// `@` — scheme parent declaration: `scheme child @ parent { ... }`
    /// reads as "child located at parent" / "the child slot of parent."
    At,
    /// `/// _<name>` — required section marker inside facet and scheme bodies.
    /// e.g. `/// _io`, `/// _facets`, `/// _dispatch_scripts` in schemes;
    /// `/// _interface`, `/// _state`, `/// _handlers` in facets.
    /// The name is the leading `_` plus the identifier; lexed without
    /// the leading underscore so the token carries just `"io"`, `"state"`, etc.
    SectionMarker(String),
    /// Internal: regular `///` comment, filtered out before tokens reach the parser.
    Comment,
}

pub type Span = SimpleSpan<usize>;
pub type Spanned<T> = (T, Span);

/// Display tokens the way they appear in source — error messages like
/// "expected `}`, found `~`" need this to read naturally.
impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s: &str = match self {
            Token::KwFacet => "facet",
            Token::KwScheme => "scheme",
            Token::KwOn => "on",
            Token::KwParallel => "parallel",
            Token::KwSequence => "sequence",
            Token::KwAsync => "async",
            Token::KwIf => "if",
            Token::KwElse => "else",
            Token::KwMatch => "match",
            Token::Ident(name) => return write!(f, "{name}"),
            Token::IntLit(n) => return write!(f, "{n}"),
            Token::FloatLit(n) => return write!(f, "{n}"),
            Token::StrLit(s) => return write!(f, "\"{s}\""),
            Token::Tilde => "~",
            Token::Colon => ":",
            Token::Equals => "=",
            Token::EqEq => "==",
            Token::Lt => "<",
            Token::Gt => ">",
            Token::Plus => "+",
            Token::Minus => "-",
            Token::Star => "*",
            Token::Comma => ",",
            Token::Arrow => "->",
            Token::TildeArrow => "~>",
            Token::Pipe => "|>",
            Token::LBrace => "{",
            Token::RBrace => "}",
            Token::LParen => "(",
            Token::RParen => ")",
            Token::LBracket => "[",
            Token::RBracket => "]",
            Token::Slash => "/",
            Token::Dot => ".",
            Token::At => "@",
            Token::SectionMarker(name) => return write!(f, "/// _{name}"),
            Token::Comment => "///",
        };
        f.write_str(s)
    }
}

/// Lex `source` into a stream of `(Token, Span)` pairs. Regular `///`
/// comments are filtered out at this boundary; section markers
/// (`/// _<name>`) survive as real tokens.
pub fn lex(source: &str) -> Result<Vec<Spanned<Token>>, Vec<Rich<'_, char>>> {
    lexer().parse(source).into_result().map(|toks| {
        toks.into_iter()
            .filter(|(t, _)| !matches!(t, Token::Comment))
            .collect()
    })
}

fn lexer<'src>() -> impl Parser<'src, &'src str, Vec<Spanned<Token>>, extra::Err<Rich<'src, char>>>
{
    let ident = text::ident().map(|s: &str| match s {
        "facet" => Token::KwFacet,
        "scheme" => Token::KwScheme,
        "on" => Token::KwOn,
        "parallel" => Token::KwParallel,
        "sequence" => Token::KwSequence,
        "async" => Token::KwAsync,
        "if" => Token::KwIf,
        "else" => Token::KwElse,
        "match" => Token::KwMatch,
        other => Token::Ident(other.to_string()),
    });

    // Float beats int — `1.0` should not lex as `IntLit(1) Dot IntLit(0)`.
    let float = text::int(10)
        .then(just('.').then(text::digits(10)))
        .to_slice()
        .map(|s: &str| Token::FloatLit(s.parse().expect("lexed float parses as f64")));

    let int = text::int(10)
        .to_slice()
        .map(|s: &str| Token::IntLit(s.parse().expect("lexed digits parse as i64")));

    // Standard escape set: `\\` `\"` `\n` `\t` `\r`.
    let escape = just('\\').ignore_then(choice((
        just('\\').to('\\'),
        just('"').to('"'),
        just('n').to('\n'),
        just('t').to('\t'),
        just('r').to('\r'),
    )));
    let string = none_of::<_, _, extra::Err<Rich<'src, char>>>("\\\"")
        .or(escape)
        .repeated()
        .collect::<String>()
        .delimited_by(just('"'), just('"'))
        .map(Token::StrLit);

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
        just('-').to(Token::Minus),
        just('*').to(Token::Star),
        just(',').to(Token::Comma),
        just('{').to(Token::LBrace),
        just('}').to(Token::RBrace),
        just('(').to(Token::LParen),
        just(')').to(Token::RParen),
        just('[').to(Token::LBracket),
        just(']').to(Token::RBracket),
        just('/').to(Token::Slash),
        just('.').to(Token::Dot),
        just('@').to(Token::At),
    ));

    // `///` opens either a SECTION MARKER (`/// _<name>`) or a regular
    // comment. Both are lexed as tokens; comments are filtered out in
    // `lex()` before they reach the parser. Section markers survive
    // because they ARE structural — required inside facet and scheme bodies.
    //
    // Discriminator: optional inline whitespace, then `_<ident>` = marker;
    // anything else to end of line = comment.
    let inline_ws = one_of(" \t").repeated();
    let triple_slash = just("///").ignore_then(choice((
        // Section marker: `_<ident>`. After the marker name, only inline
        // whitespace is consumed; the marker does NOT eat the rest of
        // the line. This means `/// _handlers on tick {}` lexes as
        // `SectionMarker("handlers")` followed by `KwOn`, etc.
        inline_ws
            .clone()
            .ignore_then(just('_'))
            .ignore_then(text::ident())
            .then_ignore(inline_ws.clone())
            .map(|name: &str| Token::SectionMarker(name.to_string())),
        // Regular `///` comment: consumes to end of line, filtered in `lex()`.
        any()
            .and_is(just('\n').not())
            .repeated()
            .to(Token::Comment),
    )));

    // Order matters: triple_slash before punct (so `///` isn't shadowed by `/`).
    // Float before int (so `1.0` doesn't lex as `1 . 0`). String stays
    // ahead for clarity.
    let token = choice((triple_slash, string, float, int, ident, punct));

    token
        .map_with(|tok, e| (tok, e.span()))
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
    fn lexes_facet_keyword() {
        assert_eq!(toks("facet"), vec![Token::KwFacet]);
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
    fn lexes_facet_with_state_field() {
        assert_eq!(
            toks("facet Hunger { ~ level: int }"),
            vec![
                Token::KwFacet,
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
            toks("/// the urchin's smallest facet\nfacet Hunger {}"),
            vec![
                Token::KwFacet,
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
    fn lexes_scheme_keyword() {
        assert_eq!(toks("scheme"), vec![Token::KwScheme]);
    }

    #[test]
    fn lexes_dispatch_modes() {
        assert_eq!(
            toks("parallel sequence async"),
            vec![Token::KwParallel, Token::KwSequence, Token::KwAsync]
        );
    }

    #[test]
    fn lexes_section_marker() {
        assert_eq!(
            toks("/// _io"),
            vec![Token::SectionMarker("io".into())]
        );
    }

    #[test]
    fn section_marker_captures_name_without_leading_underscore() {
        match &toks("/// _dispatch_scripts")[0] {
            Token::SectionMarker(name) => assert_eq!(name, "dispatch_scripts"),
            other => panic!("expected SectionMarker, got {other:?}"),
        }
    }

    #[test]
    fn section_marker_followed_by_code_lexes_cleanly() {
        // After the marker, the next tokens come from the source directly —
        // markers consume the marker name + trailing inline whitespace, NOT
        // the rest of the line. Lets single-line `facet X { /// _handlers on
        // Tick {} }` work.
        assert_eq!(
            toks("/// _handlers on tick"),
            vec![
                Token::SectionMarker("handlers".into()),
                Token::KwOn,
                Token::Ident("tick".into()),
            ]
        );
    }

    #[test]
    fn regular_comment_still_skipped() {
        // No leading `_` after `///` → comment, filtered out by lex().
        assert_eq!(
            toks("/// the urchin's smallest facet\nfacet x {}"),
            vec![
                Token::KwFacet,
                Token::Ident("x".into()),
                Token::LBrace,
                Token::RBrace,
            ]
        );
    }

    #[test]
    fn section_marker_and_following_tokens() {
        assert_eq!(
            toks("/// _state\nlevel: float"),
            vec![
                Token::SectionMarker("state".into()),
                Token::Ident("level".into()),
                Token::Colon,
                Token::Ident("float".into()),
            ]
        );
    }

    #[test]
    fn lexes_slash() {
        assert_eq!(toks("/"), vec![Token::Slash]);
    }

    #[test]
    fn triple_slash_still_lexes_as_comment_not_three_slashes() {
        // `///` is matched as a comment by the outer padding pass,
        // so it never produces three Slash tokens.
        assert_eq!(toks("/// hi\n42"), vec![Token::IntLit(42)]);
    }

    #[test]
    fn lexes_float_literal() {
        assert_eq!(toks("0.01"), vec![Token::FloatLit(0.01)]);
        assert_eq!(toks("1.0 42.5"), vec![Token::FloatLit(1.0), Token::FloatLit(42.5)]);
    }

    #[test]
    fn float_beats_int_dot_int() {
        // `1.0` must be one FloatLit, not `IntLit(1) Dot IntLit(0)`.
        assert_eq!(toks("1.0"), vec![Token::FloatLit(1.0)]);
    }

    #[test]
    fn int_alone_still_lexes_as_int() {
        assert_eq!(toks("42"), vec![Token::IntLit(42)]);
    }

    #[test]
    fn lexes_string_literal() {
        assert_eq!(toks("\"hello\""), vec![Token::StrLit("hello".into())]);
    }

    #[test]
    fn lexes_string_with_escapes() {
        assert_eq!(
            toks(r#""he said \"hi\"\n""#),
            vec![Token::StrLit("he said \"hi\"\n".into())]
        );
    }

    #[test]
    fn lexes_arithmetic_ops() {
        assert_eq!(
            toks("+ - * /"),
            vec![Token::Plus, Token::Minus, Token::Star, Token::Slash]
        );
    }

    #[test]
    fn lexes_effect_signature_separator() {
        assert_eq!(
            toks("Url -> Result / {io.http}"),
            vec![
                Token::Ident("Url".into()),
                Token::Arrow,
                Token::Ident("Result".into()),
                Token::Slash,
                Token::LBrace,
                Token::Ident("io".into()),
                Token::Dot,
                Token::Ident("http".into()),
                Token::RBrace,
            ]
        );
    }

    #[test]
    fn event_and_method_are_plain_idents() {
        // After the io decl redesign, `event` and `method` are no longer
        // reserved — the io decl uses sectional disambiguation
        // (_requests / _events) instead of inline keywords. So users can
        // name a facet field or method `event` without trouble.
        assert_eq!(toks("event"), vec![Token::Ident("event".to_string())]);
        assert_eq!(toks("method"), vec![Token::Ident("method".to_string())]);
    }

    #[test]
    fn io_remains_an_ident_not_a_keyword() {
        // `io.sim.clock` paths depend on `io` staying an ident — the io
        // decl form is matched by ident-equality in the parser, not by
        // a reserved token.
        assert_eq!(toks("io"), vec![Token::Ident("io".into())]);
    }
}
