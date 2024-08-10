use crate::tokens::Token;

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
fn str_tokens(s: &str) -> Result<Vec<Token>, TokenError> {
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
        ".*+-=,:;/\\~!(){}<>",
        Token::Dot,
        Token::Star,
        Token::Plus,
        Token::Dash,
        Token::Equals,
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
    assert_tokens!("\"hei\"", Token::DoubleQuoteLiteral);
    assert!(str_tokens("\"hei").is_err());
    assert!(str_tokens("\"hei\n\"").is_err());
    assert_tokens!("\"😀 普通话 abc普通话😀\"", Token::DoubleQuoteLiteral);

    // TODO: Escape characters
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
