use crate::{error::RainError, span::Span, tokens::peek_stream::PeekTokenStream};

use super::{expr::Expr, Ast};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match<'a> {
    expr: Box<Expr<'a>>,
}

impl<'a> Match<'a> {
    pub fn parse_stream(_stream: &mut PeekTokenStream<'a>) -> Result<Self, RainError> {
        todo!()
    }
}

impl Ast for Match<'_> {
    fn span(&self) -> Span {
        todo!()
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
