use crate::{
    error::RainError,
    tokens::{Token, TokenSpan},
};

use super::{ident::Ident, stmt::Stmt};

#[derive(Debug, PartialEq, Eq)]
pub struct FnDef<'a> {
    pub name: Ident<'a>,
    pub args: Vec<FnDefArg<'a>>,
    pub statements: Vec<Stmt<'a>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct FnDefArg<'a> {
    pub name: Ident<'a>,
}

impl<'a> FnDef<'a> {
    pub fn parse(tokens: &[TokenSpan<'a>]) -> Result<Self, RainError> {
        if tokens[0].token != Token::Fn {
            return Err(RainError::new(
                super::ParseError::ExpectedFn,
                tokens[0].span,
            ));
        }
        let Token::Ident(_) = tokens[1].token else {
            return Err(RainError::new(
                super::ParseError::ExpectedIdent,
                tokens[1].span,
            ));
        };
        if tokens[2].token != Token::LParen {
            return Err(RainError::new(
                super::ParseError::ExpectedLParen,
                tokens[2].span,
            ));
        }
        let last_token = &tokens[tokens.len() - 1];
        if last_token.token != Token::RBrace {
            return Err(RainError::new(
                super::ParseError::ExpectedRBrace,
                last_token.span,
            ));
        }
        Ok(Self {
            name: Ident::parse(tokens[1].clone())?,
            args: Vec::default(),
            statements: Vec::default(),
        })
    }

    pub fn reset_spans(&mut self) {
        for a in &mut self.args {
            a.reset_spans();
        }
        for s in &mut self.statements {
            s.reset_spans();
        }
    }
}

impl<'a> FnDefArg<'a> {
    pub fn reset_spans(&mut self) {}
}
