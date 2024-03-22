use crate::{
    error::RainError,
    tokens::{peek_stream::PeekTokenStream, TokenKind},
};

use super::{expr::Expr, helpers::PeekTokenStreamHelpers, Ast};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Return<'a> {
    pub expr: Expr<'a>,
}

impl<'a> Return<'a> {
    pub fn parse_stream(stream: &mut PeekTokenStream<'a>) -> Result<Self, RainError> {
        stream.expect_parse_next(TokenKind::Return)?;
        let expr = Expr::parse_stream(stream)?;
        Ok(Self { expr })
    }
}

impl Ast for Return<'_> {
    fn reset_spans(&mut self) {
        self.expr.reset_spans()
    }
}
