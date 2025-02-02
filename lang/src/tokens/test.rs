use quickcheck::TestResult;

use crate::{
    local_span::{ErrorLocalSpan, LocalSpan},
    tokens::Token,
};

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
fn single_quote_literal() {
    assert_tokens!("'hei'", Token::SingleQuoteLiteral(None));
    assert!(str_tokens("'hei").is_err());
    assert!(str_tokens("'hei\n'").is_err());
    assert_tokens!("'😀 普通话 abc普通话😀'", Token::SingleQuoteLiteral(None));
    assert_tokens!(
        "f'{aljskdfa}'",
        Token::SingleQuoteLiteral(Some(crate::tokens::StringLiteralPrefix::Format))
    );
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
    assert_tokens!("\"he\\\"i\"", Token::DoubleQuoteLiteral(None));
    assert_tokens!("\"he\\ni\"", Token::DoubleQuoteLiteral(None));
    assert_tokens!("\"he\\\ni\"", Token::DoubleQuoteLiteral(None));
}

#[test]
fn keywords() {
    assert_tokens!(
        "fn let pub if else true false internal",
        Token::Fn,
        Token::Let,
        Token::Pub,
        Token::If,
        Token::Else,
        Token::True,
        Token::False,
        Token::Internal
    );
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

#[test]
fn illegal_chars() {
    assert_eq!(
        str_tokens("`"),
        Err(LocalSpan::byte(0).with_error(TokenError::IllegalChar))
    );
}

#[expect(clippy::needless_pass_by_value)]
#[quickcheck_macros::quickcheck]
fn tokenise_any_script(src: String) {
    let _: Result<(), ErrorLocalSpan<TokenError>> =
        TokenStream::new(&src).try_for_each(|r| r.map(|_| ()));
}

#[expect(clippy::needless_pass_by_value)]
#[quickcheck_macros::quickcheck]
fn tokenise_non_control_character_script(src: String) -> TestResult {
    if src.contains(|c: char| match c {
        '"' | '\'' | '`' => true,
        _ => c.is_control(),
    }) {
        return TestResult::discard();
    }
    convert_test_result(TokenStream::new(&src).try_for_each(|r| {
        r.map(|tls| {
            // Check the span can be indexed and doesn't break UTF-8 boundaries
            tls.span.contents(&src);
        })
    }))
}

#[expect(clippy::needless_pass_by_value)]
#[quickcheck_macros::quickcheck]
fn tokenise_string_literal(contents: String) -> TestResult {
    if contents.contains(['"', '\n', '\\']) {
        return TestResult::discard();
    }
    let literal = format!("\"{contents}\"");
    convert_test_result(TokenStream::new(&literal).try_for_each(|r| r.map(|_| ())))
}

fn convert_test_result<T, E: std::fmt::Debug>(res: Result<T, E>) -> TestResult {
    match res {
        Ok(_) => TestResult::passed(),
        Err(err) => TestResult::error(format!("{err:?}")),
    }
}
