use crate::{
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, NextTokenSpan, Token},
};

use super::{statement::Statement, Ast};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementList {
    pub statements: Vec<Statement>,
}

impl StatementList {
    pub fn parse_stream(stream: &mut PeekTokenStream) -> Result<Self, RainError> {
        let mut statements = Vec::new();
        loop {
            let peeking = stream.peek()?;
            let NextTokenSpan::Next(token) = peeking.value() else {
                break;
            };
            match token.token {
                Token::NewLine | Token::Comment(_) => {
                    peeking.consume();
                    continue;
                }
                Token::RBrace => {
                    break;
                }
                _ => (),
            }
            statements.push(Statement::parse_stream(stream)?);
        }
        Ok(Self { statements })
    }

    pub fn nosp(statements: Vec<Statement>) -> Self {
        Self { statements }
    }
}

impl Ast for StatementList {
    fn span(&self) -> Span {
        todo!("statement list span")
    }

    fn reset_spans(&mut self) {
        for s in &mut self.statements {
            s.reset_spans();
        }
    }
}
