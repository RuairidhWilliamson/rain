use crate::{
    error::RainError,
    tokens::{peek_stream::PeekTokenStream, NextTokenSpan, Token},
};

use super::{statement::Statement, Ast};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementList<'a> {
    pub statements: Vec<Statement<'a>>,
}

impl<'a> StatementList<'a> {
    pub fn parse_stream(stream: &mut PeekTokenStream<'a>) -> Result<Self, RainError> {
        let mut statements = Vec::new();
        loop {
            let peeking = stream.peek()?;
            let NextTokenSpan::Next(token) = peeking.value() else {
                break;
            };
            if token.token == Token::NewLine {
                peeking.consume();
                continue;
            } else if token.token == Token::RBrace {
                break;
            }
            statements.push(Statement::parse_stream(stream)?);
        }
        Ok(Self { statements })
    }

    pub fn nosp(statements: Vec<Statement<'a>>) -> Self {
        Self { statements }
    }
}

impl Ast for StatementList<'_> {
    fn reset_spans(&mut self) {
        for s in &mut self.statements {
            s.reset_spans();
        }
    }
}
