use crate::{
    error::RainError,
    tokens::{Token, TokenSpan},
};

use super::{expr::Expr, ParseError};

#[derive(Debug, PartialEq, Eq)]
pub struct Declare<'a> {
    pub name: &'a str,
    pub value: Expr<'a>,
}

impl<'a> Declare<'a> {
    pub fn parse(tokens: &[TokenSpan<'a>]) -> Result<Self, RainError> {
        assert_eq!(tokens[0].token, Token::Let);
        let Token::Ident(name) = tokens[1].token else {
            panic!("expected ident in let statement");
        };
        if tokens[2].token != Token::Assign {
            return Err(RainError::new(
                ParseError::ExpectedAssignToken,
                TokenSpan::span(tokens).unwrap(),
            ));
        }
        let value = Expr::parse(&tokens[3..], TokenSpan::span(tokens).unwrap())?;
        Ok(Self { name, value })
    }

    pub fn reset_spans(&mut self) {
        self.value.reset_spans();
    }
}
