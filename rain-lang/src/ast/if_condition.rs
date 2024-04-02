use crate::{
    ast::block::Block,
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, NextTokenSpan, Token, TokenKind},
};

use super::{expr::Expr, helpers::PeekTokenStreamHelpers, Ast};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfCondition {
    pub if_token: Span,
    pub condition: Box<Expr>,
    pub then_block: Block,
    pub else_condition: Option<ElseCondition>,
}

impl IfCondition {
    pub fn parse_stream(stream: &mut PeekTokenStream) -> Result<Self, RainError> {
        let if_token = stream.expect_parse_next(TokenKind::If)?.span;
        let condition = Box::new(Expr::parse_stream(stream)?);
        let then_block = Block::parse_stream(stream)?;
        let else_condition = if let NextTokenSpan::Next(token_span) = stream.peek()?.value() {
            if token_span.token == Token::Else {
                Some(ElseCondition::parse_stream(stream)?)
            } else {
                None
            }
        } else {
            None
        };
        Ok(Self {
            if_token,
            condition,
            then_block,
            else_condition,
        })
    }

    pub fn nosp(condition: Expr, then_block: Block, else_condition: Option<ElseCondition>) -> Self {
        Self {
            if_token: Span::default(),
            condition: Box::new(condition),
            then_block,
            else_condition,
        }
    }
}

impl Ast for IfCondition {
    fn span(&self) -> Span {
        let last = self
            .else_condition
            .as_ref()
            .map(|e| e.span())
            .unwrap_or_else(|| self.then_block.span());
        self.if_token.combine(last)
    }

    fn reset_spans(&mut self) {
        self.if_token.reset();
        self.condition.reset_spans();
        self.then_block.reset_spans();
        if let Some(e) = self.else_condition.as_mut() {
            e.reset_spans();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElseCondition {
    pub else_token: Span,
    pub block: Block,
}

impl ElseCondition {
    pub fn parse_stream(stream: &mut PeekTokenStream) -> Result<Self, RainError> {
        let else_token = stream.expect_parse_next(TokenKind::Else)?.span;
        let else_block = Block::parse_stream(stream)?;
        Ok(Self {
            else_token,
            block: else_block,
        })
    }

    pub fn nosp(block: Block) -> Self {
        Self {
            else_token: Span::default(),
            block,
        }
    }
}

impl Ast for ElseCondition {
    fn span(&self) -> Span {
        self.else_token.combine(self.block.span())
    }

    fn reset_spans(&mut self) {
        self.else_token.reset();
        self.block.reset_spans();
    }
}
