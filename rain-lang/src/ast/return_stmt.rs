use crate::{
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, TokenKind},
};

use super::{expr::Expr, helpers::PeekTokenStreamHelpers, Ast};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Return<'a> {
    pub return_token: Span,
    pub expr: Expr<'a>,
}

impl<'a> Return<'a> {
    pub fn parse_stream(stream: &mut PeekTokenStream<'a>) -> Result<Self, RainError> {
        let return_token = stream.expect_parse_next(TokenKind::Return)?.span;
        let expr = Expr::parse_stream(stream)?;
        Ok(Self { return_token, expr })
    }

    pub fn nosp(expr: Expr<'a>) -> Self {
        Self {
            return_token: Span::default(),
            expr,
        }
    }
}

impl Ast for Return<'_> {
    fn span(&self) -> Span {
        self.return_token.combine(self.expr.span())
    }

    fn reset_spans(&mut self) {
        self.return_token.reset();
        self.expr.reset_spans()
    }
}
