use crate::{
    error::RainError,
    span::Span,
    tokens::{Token, TokenSpan},
};

use super::ident::Ident;

#[derive(Debug, PartialEq, Eq)]
pub struct Item<'a> {
    pub idents: Vec<Ident<'a>>,
    pub span: Span,
}

impl<'a> Item<'a> {
    pub fn parse(tokens: &[TokenSpan<'a>]) -> Result<Self, RainError> {
        let idents = tokens
            .iter()
            .filter_map(|t| match &t.token {
                Token::Ident(ident) => Some(Ident {
                    name: ident,
                    span: t.span,
                }),
                Token::Dot => None,
                token => panic!("unexpected token {token:?}"),
            })
            .collect();
        let span = tokens
            .iter()
            .map(|ts| ts.span)
            .reduce(Span::combine)
            .unwrap();
        Ok(Self { idents, span })
    }

    pub fn reset_spans(&mut self) {
        self.idents.iter_mut().for_each(|ident| ident.span_reset());
        self.span.reset();
    }
}
