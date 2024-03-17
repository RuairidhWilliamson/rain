pub mod peek_stream;
pub mod stream;

use crate::span::{Place, Span};

#[derive(Debug, Clone, PartialEq, Eq, enum_kinds::EnumKind)]
#[enum_kind(TokenKind)]
pub enum Token<'a> {
    Ident(&'a str),
    DoubleQuoteLiteral(&'a str),
    TrueLiteral,
    FalseLiteral,
    Let,
    If,
    Else,
    Fn,
    Dot,
    Assign,
    Comma,
    Colon,
    Slash,
    Tilde,
    Return,
    LParen,
    RParen,
    LBrace,
    RBrace,
    NewLine,
}

#[derive(Debug, Clone)]
pub struct TokenSpan<'a> {
    pub token: Token<'a>,
    pub span: Span,
}

impl TokenSpan<'_> {
    pub fn span(tokens: &[Self]) -> Option<Span> {
        Some(Span::combine(tokens.first()?.span, tokens.last()?.span))
    }
}

pub enum NextTokenSpan<'a> {
    Next(TokenSpan<'a>),
    End(Span),
}

#[derive(Debug)]
pub struct TokenError {
    pub char: Option<char>,
    pub place: Place,
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.char, f)
    }
}

impl TokenError {
    pub fn span(&self) -> Span {
        Span::new_single_byte(self.place)
    }
}

#[cfg(test)]
mod tests {
    use crate::tokens::stream::TokenStream;

    use super::Token;

    #[test]
    fn tokens_assignment() {
        let source = "let a = \"abc\"";
        let tokens: Vec<Token> = TokenStream::new(source)
            .map(|ts| ts.unwrap().token)
            .collect();

        assert_eq!(
            tokens,
            vec![
                Token::Let,
                Token::Ident("a"),
                Token::Assign,
                Token::DoubleQuoteLiteral("abc")
            ],
        )
    }

    #[test]
    fn token_fn_declaration() {
        let source = "fn foo() {
            core.print(\"Hello :)\")
        }";
        let tokens: Vec<Token> = TokenStream::new(source)
            .map(|ts| ts.unwrap().token)
            .filter(|t| !matches!(t, Token::NewLine))
            .collect();
        assert_eq!(
            tokens,
            vec![
                Token::Fn,
                Token::Ident("foo"),
                Token::LParen,
                Token::RParen,
                Token::LBrace,
                Token::Ident("core"),
                Token::Dot,
                Token::Ident("print"),
                Token::LParen,
                Token::DoubleQuoteLiteral("Hello :)"),
                Token::RParen,
                Token::RBrace,
            ]
        )
    }

    #[test]
    fn tokens_hello_world() {
        let source = "core.print(\"hello world\")";
        let tokens: Vec<Token> = TokenStream::new(source)
            .map(|ts| ts.unwrap().token)
            .collect();

        assert_eq!(
            tokens,
            vec![
                Token::Ident("core"),
                Token::Dot,
                Token::Ident("print"),
                Token::LParen,
                Token::DoubleQuoteLiteral("hello world"),
                Token::RParen
            ]
        );
    }

    #[test]
    fn tokens_multiline() {
        let source = "core.print()\ncore.print()";
        let tokens: Vec<Token> = TokenStream::new(source)
            .map(|ts| ts.unwrap().token)
            .collect();

        assert_eq!(
            tokens,
            vec![
                Token::Ident("core"),
                Token::Dot,
                Token::Ident("print"),
                Token::LParen,
                Token::RParen,
                Token::NewLine,
                Token::Ident("core"),
                Token::Dot,
                Token::Ident("print"),
                Token::LParen,
                Token::RParen,
            ]
        );
    }

    #[test]
    fn tokens_comment() {
        let source = "core.print()\n# This should not be tokens\ncore.print()";
        let tokens: Vec<Token> = TokenStream::new(source)
            .map(|ts| ts.unwrap().token)
            .collect();

        assert_eq!(
            tokens,
            vec![
                Token::Ident("core"),
                Token::Dot,
                Token::Ident("print"),
                Token::LParen,
                Token::RParen,
                Token::NewLine,
                Token::NewLine,
                Token::Ident("core"),
                Token::Dot,
                Token::Ident("print"),
                Token::LParen,
                Token::RParen,
            ]
        )
    }

    #[test]
    fn tokens_emoji() {
        let source = "let 🦀 = \"🦀\"";
        let tokens: Vec<Token> = TokenStream::new(source)
            .map(|ts| ts.unwrap().token)
            .collect();
        assert_eq!(
            tokens,
            vec![
                Token::Let,
                Token::Ident("🦀"),
                Token::Assign,
                Token::DoubleQuoteLiteral("🦀"),
            ]
        )
    }

    #[test]
    fn tokens_emoji2() {
        let source = "let 🌧 = \"rain\"";
        let tokens: Vec<Token> = TokenStream::new(source)
            .map(|ts| ts.unwrap().token)
            .collect();
        assert_eq!(
            tokens,
            vec![
                Token::Let,
                Token::Ident("🌧"),
                Token::Assign,
                Token::DoubleQuoteLiteral("rain"),
            ]
        )
    }

    #[test]
    fn tokens_column() {
        let source = "core.print(\"hello world\")";
        let token_span = TokenStream::new(source).last().unwrap().unwrap();
        assert_eq!(token_span.span.start.column, source.len() - 1);
    }
}
