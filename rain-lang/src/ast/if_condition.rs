use crate::{
    ast::block::Block,
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, NextTokenSpan, Token, TokenKind},
};

use super::{expr::Expr, helpers::PeekTokenStreamHelpers, Ast};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfCondition<'a> {
    pub if_token: Span,
    pub condition: Box<Expr<'a>>,
    pub then_block: Block<'a>,
    pub else_condition: Option<ElseCondition<'a>>,
}

impl<'a> IfCondition<'a> {
    pub fn parse_stream(stream: &mut PeekTokenStream<'a>) -> Result<Self, RainError> {
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

    pub fn nosp(
        condition: Expr<'a>,
        then_block: Block<'a>,
        else_condition: Option<ElseCondition<'a>>,
    ) -> Self {
        Self {
            if_token: Span::default(),
            condition: Box::new(condition),
            then_block,
            else_condition,
        }
    }
}

impl Ast for IfCondition<'_> {
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
pub struct ElseCondition<'a> {
    pub else_token: Span,
    pub block: Block<'a>,
}

impl<'a> ElseCondition<'a> {
    pub fn parse_stream(stream: &mut PeekTokenStream<'a>) -> Result<Self, RainError> {
        let else_token = stream.expect_parse_next(TokenKind::Else)?.span;
        let else_block = Block::parse_stream(stream)?;
        Ok(Self {
            else_token,
            block: else_block,
        })
    }

    pub fn nosp(block: Block<'a>) -> Self {
        Self {
            else_token: Span::default(),
            block,
        }
    }
}

impl Ast for ElseCondition<'_> {
    fn span(&self) -> Span {
        self.else_token.combine(self.block.span())
    }

    fn reset_spans(&mut self) {
        self.else_token.reset();
        self.block.reset_spans();
    }
}
