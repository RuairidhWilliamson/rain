use crate::{
    error::RainError,
    span::Span,
    tokens::{Token, TokenKind, TokenSpan},
};

use super::ParseError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident<'a> {
    pub name: &'a str,
    pub span: Span,
}

impl<'a> Ident<'a> {
    pub fn parse(token: TokenSpan<'a>) -> Result<Self, RainError> {
        let Token::Ident(name) = token.token else {
            return Err(RainError::new(
                ParseError::Expected(TokenKind::Ident),
                token.span,
            ));
        };
        Ok(Self {
            name,
            span: token.span,
        })
    }

    // Creates an Ident with a default span
    pub fn nosp(name: &'a str) -> Self {
        Self {
            name,
            span: Span::default(),
        }
    }

    pub fn span_reset(&mut self) {
        self.span.reset();
    }
}
