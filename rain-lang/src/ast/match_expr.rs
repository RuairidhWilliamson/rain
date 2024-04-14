use crate::{error::RainError, span::Span, tokens::peek_stream::PeekTokenStream};

use super::{expr::Expr, Ast};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    expr: Box<Expr>,
}

impl Match {
    pub fn parse_stream(_stream: &mut PeekTokenStream) -> Result<Self, RainError> {
        todo!("parse match expr")
    }
}

impl Ast for Match {
    fn span(&self) -> Span {
        todo!("match span")
    }

    fn reset_spans(&mut self) {
        self.expr.reset_spans();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn parse_empty_match() {}
}
