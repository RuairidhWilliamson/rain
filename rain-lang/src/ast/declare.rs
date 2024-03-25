use crate::{
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, TokenKind},
};

use super::{
    expr::Expr,
    helpers::{NextTokenSpanHelpers, PeekTokenStreamHelpers},
    ident::Ident,
    Ast,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declare<'a> {
    pub kind: DeclareKind,
    pub token: Span,
    pub name: Ident<'a>,
    pub equals_token: Span,
    pub value: Expr<'a>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeclareKind {
    Let,
    Lazy,
}

impl<'a> Declare<'a> {
    pub fn parse_stream(stream: &mut PeekTokenStream<'a>) -> Result<Self, RainError> {
        let token = stream
            .parse_next()?
            .expect_next_any(&[TokenKind::Let, TokenKind::Lazy])?;
        let kind = match token.token.kind() {
            TokenKind::Let => DeclareKind::Let,
            TokenKind::Lazy => DeclareKind::Lazy,
            _ => unreachable!("expect_next_any only allows let or lazy"),
        };
        let ident_token = stream.expect_parse_next(TokenKind::Ident)?;
        let name = Ident::parse(ident_token)?;
        let equals_token = stream.expect_parse_next(TokenKind::Equals)?.span;
        let value = Expr::parse_stream(stream)?;
        Ok(Self {
            kind,
            token: token.span,
            name,
            equals_token,
            value,
        })
    }

    pub fn nosp(kind: DeclareKind, name: Ident<'a>, value: Expr<'a>) -> Self {
        Self {
            kind,
            token: Span::default(),
            name,
            equals_token: Span::default(),
            value,
        }
    }
}

impl Ast for Declare<'_> {
    fn span(&self) -> Span {
        self.token.combine(self.value.span())
    }

    fn reset_spans(&mut self) {
        self.token.reset();
        self.name.reset_spans();
        self.equals_token.reset();
        self.value.reset_spans();
    }
}
