use crate::{
    error::RainError,
    span::Span,
    tokens::{Token, TokenKind, TokenSpan},
};

use super::{Ast, ParseError};

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
}

impl Ast for Ident<'_> {
    fn span(&self) -> Span {
        self.span
    }

    fn reset_spans(&mut self) {
        self.span.reset();
    }
}
