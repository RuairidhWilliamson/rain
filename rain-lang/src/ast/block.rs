use crate::{
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, TokenKind},
};

use super::{
    helpers::PeekTokenStreamHelpers, statement::Statement, statement_list::StatementList, Ast,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block<'a> {
    pub lbrace_token: Span,
    pub stmts: StatementList<'a>,
    pub rbrace_token: Span,
}

impl<'a> Block<'a> {
    pub fn parse_stream(stream: &mut PeekTokenStream<'a>) -> Result<Self, RainError> {
        let lbrace_token = stream.expect_parse_next(TokenKind::LBrace)?.span;
        let stmts = StatementList::parse_stream(stream)?;
        let rbrace_token = stream.expect_parse_next(TokenKind::RBrace)?.span;
        Ok(Self {
            lbrace_token,
            stmts,
            rbrace_token,
        })
    }

    pub fn nosp(stmts: Vec<Statement<'a>>) -> Self {
        Self {
            lbrace_token: Span::default(),
            stmts: StatementList::nosp(stmts),
            rbrace_token: Span::default(),
        }
    }
}

impl Ast for Block<'_> {
    fn reset_spans(&mut self) {
        self.lbrace_token.reset();
        self.stmts.reset_spans();
        self.rbrace_token.reset();
    }
}
