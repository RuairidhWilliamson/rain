use crate::{
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, TokenKind},
};

use super::{expr::Expr, helpers::PeekTokenStreamHelpers, ident::Ident, Ast};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declare<'a> {
    pub let_token: Span,
    pub name: Ident<'a>,
    pub equals_token: Span,
    pub value: Expr<'a>,
}

impl<'a> Declare<'a> {
    pub fn parse_stream(stream: &mut PeekTokenStream<'a>) -> Result<Self, RainError> {
        let let_token = stream.expect_parse_next(TokenKind::Let)?.span;
        let ident_token = stream.expect_parse_next(TokenKind::Ident)?;
        let name = Ident::parse(ident_token)?;
        let equals_token = stream.expect_parse_next(TokenKind::Equals)?.span;
        let value = Expr::parse_stream(stream)?;
        Ok(Self {
            let_token,
            name,
            equals_token,
            value,
        })
    }

    pub fn nosp(name: Ident<'a>, value: Expr<'a>) -> Self {
        Self {
            let_token: Span::default(),
            name,
            equals_token: Span::default(),
            value,
        }
    }
}

impl Ast for Declare<'_> {
    fn span(&self) -> Span {
        self.let_token.combine(self.value.span())
    }

    fn reset_spans(&mut self) {
        self.let_token.reset();
        self.name.reset_spans();
        self.equals_token.reset();
        self.value.reset_spans();
    }
}
