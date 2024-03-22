use crate::{
    ast::block::Block,
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, TokenKind},
};

use super::{expr::Expr, helpers::PeekTokenStreamHelpers, Ast};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfCondition<'a> {
    pub if_token: Span,
    pub condition: Box<Expr<'a>>,
    pub block: Block<'a>,
}

impl<'a> IfCondition<'a> {
    pub fn parse_stream(stream: &mut PeekTokenStream<'a>) -> Result<Self, RainError> {
        let if_token = stream.expect_parse_next(TokenKind::If)?.span;
        let condition = Box::new(Expr::parse_stream(stream)?);
        let block = Block::parse_stream(stream)?;
        Ok(Self {
            if_token,
            condition,
            block,
        })
    }

    pub fn nosp(condition: Expr<'a>, block: Block<'a>) -> Self {
        Self {
            if_token: Span::default(),
            condition: Box::new(condition),
            block,
        }
    }
}

impl Ast for IfCondition<'_> {
    fn reset_spans(&mut self) {
        self.if_token.reset();
        self.condition.reset_spans();
        self.block.reset_spans();
    }
}
