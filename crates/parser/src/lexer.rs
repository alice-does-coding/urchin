//! Lexer — turns source text into a stream of `(Token, Span)`.
//!
//! Uses chumsky character-level combinators. Comments (`///` … to end of line)
//! and whitespace are skipped; everything else becomes a token.

use chumsky::prelude::*;
use chumsky::span::SimpleSpan;

/// Tokens for the minimal Urchin subset. Grows as the grammar grows.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Token {
    /// `role`
    KwRole,
    /// PascalCase or snake_case identifier.
    Ident(String),
    /// `~`
    Tilde,
    /// `:`
    Colon,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
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
        other => Token::Ident(other.to_string()),
    });

    let punct = choice((
        just('~').to(Token::Tilde),
        just(':').to(Token::Colon),
        just('{').to(Token::LBrace),
        just('}').to(Token::RBrace),
        just('.').to(Token::Dot),
    ));

    let token = choice((ident, punct));

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
}
