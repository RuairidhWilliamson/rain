use crate::{
    error::RainError,
    tokens::{peek_stream::PeekTokenStream, TokenKind},
};

use super::{expr::Expr, helpers::PeekTokenStreamHelpers, ident::Ident, Ast};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declare<'a> {
    pub name: Ident<'a>,
    pub value: Expr<'a>,
}

impl<'a> Declare<'a> {
    pub fn parse_stream(stream: &mut PeekTokenStream<'a>) -> Result<Self, RainError> {
        stream.expect_parse_next(TokenKind::Let)?;
        let ident_token = stream.expect_parse_next(TokenKind::Ident)?;
        let name = Ident::parse(ident_token)?;
        stream.expect_parse_next(TokenKind::Equals)?;
        let value = Expr::parse_stream(stream)?;
        Ok(Self { name, value })
    }
}

impl Ast for Declare<'_> {
    fn reset_spans(&mut self) {
        self.name.reset_spans();
        self.value.reset_spans();
    }
}
