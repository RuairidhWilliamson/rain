use crate::{error::RainError, span::Span, tokens::peek_stream::PeekTokenStream};

use super::{expr::Expr, Ast};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operator {
    Equals,
    NotEquals,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryInfixOperator {
    pub left: Box<Expr>,
    pub operator: Operator,
    token: Span,
    pub right: Box<Expr>,
}

impl BinaryInfixOperator {
    pub fn parse_stream(_stream: &mut PeekTokenStream) -> Result<Self, RainError> {
        todo!()
    }
}

impl Ast for BinaryInfixOperator {
    fn span(&self) -> Span {
        self.left.span().combine(self.right.span())
    }

    fn reset_spans(&mut self) {
        self.left.reset_spans();
        self.token.reset();
        self.right.reset_spans();
    }
}
