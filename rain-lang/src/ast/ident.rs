use crate::{
    error::RainError,
    span::Span,
    tokens::{Token, TokenKind, TokenSpan},
};

use super::{Ast, ParseError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

impl Ident {
    pub fn parse(token: TokenSpan<'_>) -> Result<Self, RainError> {
        let Token::Ident(name) = token.token else {
            return Err(RainError::new(
                ParseError::Expected(TokenKind::Ident),
                token.span,
            ));
        };
        Ok(Self {
            name: String::from(name),
            span: token.span,
        })
    }

    // Creates an Ident with a default span
    pub fn nosp(name: &str) -> Self {
        Self {
            name: String::from(name),
            span: Span::default(),
        }
    }
}

impl Ast for Ident {
    fn span(&self) -> Span {
        self.span
    }

    fn reset_spans(&mut self) {
        self.span.reset();
    }
}
