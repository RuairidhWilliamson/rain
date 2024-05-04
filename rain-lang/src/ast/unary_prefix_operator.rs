use crate::{
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, Token, TokenKind},
};

use super::{expr::Expr, helpers::NextTokenSpanHelpers, Ast};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operator {
    Exclamation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnaryPrefixOperator {
    pub operator: Operator,
    token: Span,
    pub expr: Box<Expr>,
}

impl UnaryPrefixOperator {
    pub fn parse_stream(stream: &mut PeekTokenStream) -> Result<Self, RainError> {
        let t = stream
            .parse_next()?
            .expect_next_any(&[TokenKind::Exclamation])?;
        let operator = match t.token {
            Token::Exclamation => Operator::Exclamation,
            _ => unreachable!(),
        };
        Ok(Self {
            operator,
            token: t.span,
            expr: Box::new(Expr::parse_stream(stream)?),
        })
    }
}

impl Ast for UnaryPrefixOperator {
    fn span(&self) -> Span {
        self.token.combine(self.expr.span())
    }

    fn reset_spans(&mut self) {
        self.token.reset();
        self.expr.reset_spans();
    }
}
