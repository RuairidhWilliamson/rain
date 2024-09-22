use crate::{local_span::ErrorLocalSpan, tokens::Token};

use super::{stream::TokenStream, TokenError};

macro_rules! assert_tokens {
    ($s:expr) => {
        assert_eq!(str_tokens($s).unwrap(), vec![])
    };
    ($s:expr, $($t:expr),+) => {
        assert_eq!(str_tokens($s).unwrap(), vec![$($t),+])
    };
    ($s:expr, $($t:expr),+,) => {
        assert_tokens!($s, $($t),+)
    };
}

fn str_tokens(s: &str) -> Result<Vec<Token>, ErrorLocalSpan<TokenError>> {
    TokenStream::new(s)
        .map(|res| res.map(|tls| tls.token))
        .collect()
}

#[test]
fn empty() {
    assert_tokens!("");
}

#[test]
fn symbols() {
    assert_tokens!(
        ".*+-=,:;/\\~!(){}<>&|?@%$^",
        Token::Dot,
        Token::Star,
        Token::Plus,
        Token::Subtract,
        Token::Assign,
        Token::Comma,
        Token::Colon,
        Token::Semicolon,
        Token::Slash,
        Token::Backslash,
        Token::Tilde,
        Token::Excalmation,
        Token::LParen,
        Token::RParen,
        Token::LBrace,
        Token::RBrace,
        Token::LAngle,
        Token::RAngle,
        Token::Ampersand,
        Token::Pipe,
        Token::Question,
        Token::At,
        Token::Percent,
        Token::Dollar,
        Token::Caret,
    );
}

#[test]
fn compound_symbols() {
    assert_tokens!(
        "==&&!=||",
        Token::Equals,
        Token::LogicalAnd,
        Token::NotEquals,
        Token::LogicalOr
    );
}

#[test]
fn idents() {
    assert_tokens!("foo a123 _abc_", Token::Ident, Token::Ident, Token::Ident);
    assert_tokens!(
        "😀 普通话 abc普通话😀",
        Token::Ident,
        Token::Ident,
        Token::Ident
    );
}

#[test]
fn fn_call() {
    assert_tokens!("foo()", Token::Ident, Token::LParen, Token::RParen);
}

#[test]
fn module_dot() {
    assert_tokens!("foo.bar", Token::Ident, Token::Dot, Token::Ident);
}

#[test]
fn multiline() {
    assert_tokens!("foo\nbar", Token::Ident, Token::NewLine, Token::Ident);
}

#[test]
fn space() {
    assert_tokens!("foo bar", Token::Ident, Token::Ident);
}

#[test]
fn double_quote_literal() {
    assert_tokens!("\"hei\"", Token::DoubleQuoteLiteral(None));
    assert!(str_tokens("\"hei").is_err());
    assert!(str_tokens("\"hei\n\"").is_err());
    assert_tokens!("\"😀 普通话 abc普通话😀\"", Token::DoubleQuoteLiteral(None));
    assert_tokens!(
        "f\"{aljskdfa}\"",
        Token::DoubleQuoteLiteral(Some(crate::tokens::StringLiteralPrefix::Format))
    );

    // TODO: Escape characters in string literals
    // assert_tokens!("\"he\\\"i\"", Token::DoubleQuoteLiteral);
    // assert_tokens!("\"he\\ni\"", Token::DoubleQuoteLiteral);
    // assert_tokens!("\"he\\\ni\"", Token::DoubleQuoteLiteral);
}

#[test]
fn keywords() {
    assert_tokens!("fn let", Token::Fn, Token::Let);
}

#[test]
fn number() {
    assert_tokens!(
        "1 100 59123 04",
        Token::Number,
        Token::Number,
        Token::Number,
        Token::Number,
    );
}

#[quickcheck_macros::quickcheck]
fn tokenise_any_script(src: String) {
    let _: Result<(), ErrorLocalSpan<TokenError>> =
        TokenStream::new(&src).map(|r| r.map(|_| ())).collect();
}

#[quickcheck_macros::quickcheck]
fn tokenise_non_control_character_script(src: String) -> Result<(), ErrorLocalSpan<TokenError>> {
    if src.contains(|c: char| c.is_control()) {
        return Ok(());
    }
    TokenStream::new(&src)
        .map(|r| {
            r.map(|tls| {
                // Check the span can be indexed and doesn't break UTF-8 boundaries
                tls.span.contents(&src);
            })
        })
        .collect()
}

#[quickcheck_macros::quickcheck]
fn tokenise_string_literal(contents: String) -> Result<(), ErrorLocalSpan<TokenError>> {
    if contents.contains(&['"', '\n']) {
        return Ok(());
    }
    let literal = format!("\"{contents}\"");
    TokenStream::new(&literal).map(|r| r.map(|_| ())).collect()
}
