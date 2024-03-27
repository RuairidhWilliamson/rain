use crate::{
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, TokenKind},
};

use super::{expr::Expr, helpers::PeekTokenStreamHelpers, ident::Ident, Ast};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dot<'a> {
    pub left: Option<Box<Expr<'a>>>,
    pub dot_token: Span,
    pub right: Ident<'a>,
}

impl<'a> Dot<'a> {
    pub fn parse_stream(
        left: Option<Expr<'a>>,
        stream: &mut PeekTokenStream<'a>,
    ) -> Result<Self, RainError> {
        let dot_token = stream.expect_parse_next(TokenKind::Dot)?.span;
        let right = Ident::parse(stream.expect_parse_next(TokenKind::Ident)?)?;
        Ok(Self {
            left: left.map(Box::new),
            dot_token,
            right,
        })
    }

    pub fn nosp(left: Option<Expr<'a>>, right: Ident<'a>) -> Self {
        Self {
            left: left.map(Box::new),
            dot_token: Span::default(),
            right,
        }
    }
}

impl Ast for Dot<'_> {
    fn span(&self) -> Span {
        self.left
            .as_ref()
            .map(|l| l.span())
            .unwrap_or(self.dot_token)
            .combine(self.right.span())
    }

    fn reset_spans(&mut self) {
        if let Some(l) = self.left.as_mut() {
            l.reset_spans()
        }
        self.dot_token.reset();
        self.right.reset_spans();
    }
}
