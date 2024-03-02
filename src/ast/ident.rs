use crate::{
    error::RainError,
    span::Span,
    tokens::{Token, TokenSpan},
};

#[derive(Debug, PartialEq, Eq)]
pub struct Ident<'a> {
    pub name: &'a str,
    pub span: Span,
}

impl<'a> Ident<'a> {
    pub fn parse(token: &TokenSpan<'a>) -> Result<Self, RainError> {
        let Token::Ident(name) = token.token else {
            return Err(RainError::new(super::ParseError::ExpectedIdent, token.span));
        };
        Ok(Self {
            name,
            span: token.span,
        })
    }

    pub fn span_reset(&mut self) {
        self.span.reset();
    }
}
