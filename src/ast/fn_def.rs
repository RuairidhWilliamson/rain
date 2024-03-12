use crate::{
    error::RainError,
    tokens::{peek_stream::PeekTokenStream, NextTokenSpan, Token, TokenKind},
};

use super::{
    helpers::{NextTokenSpanHelpers, PeekNextTokenHelpers, PeekTokenStreamHelpers},
    ident::Ident,
    stmt::Stmt,
    ParseError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnDef<'a> {
    pub name: Ident<'a>,
    pub args: Vec<FnDefArg<'a>>,
    pub statements: Vec<Stmt<'a>>,
}

impl<'a> FnDef<'a> {
    pub fn parse_stream(stream: &mut PeekTokenStream<'a>) -> Result<Self, RainError> {
        stream.expect_parse_next(TokenKind::Fn)?;
        let name = Ident::parse(stream.expect_parse_next(TokenKind::Ident)?)?;
        stream.expect_parse_next(TokenKind::LParen)?;
        let mut args = Vec::new();
        loop {
            let peeking = stream.peek()?;
            let peeking_token_span =
                peeking.expect_not_end(ParseError::Expected(TokenKind::RParen))?;
            if peeking_token_span.token == Token::RParen {
                peeking.consume();
                break;
            }
            if TokenKind::from(&peeking_token_span.token) == TokenKind::Ident {
                let ident = Ident::parse(peeking.consume().expect_next(TokenKind::Ident)?)?;
                args.push(FnDefArg { name: ident });
            }
            let peeking = stream.peek()?;
            let peeking_token_span =
                peeking.expect_not_end(ParseError::Expected(TokenKind::RParen))?;
            if peeking_token_span.token == Token::RParen {
                peeking.consume();
                break;
            } else if peeking_token_span.token == Token::Comma {
                peeking.consume();
            }
        }
        stream.expect_parse_next(TokenKind::LBrace)?;
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
            statements.push(Stmt::parse_stream(stream)?);
        }
        stream.expect_parse_next(TokenKind::RBrace)?;
        Ok(Self {
            name,
            args,
            statements,
        })
    }

    pub fn reset_spans(&mut self) {
        self.name.span_reset();
        for a in &mut self.args {
            a.reset_spans();
        }
        for s in &mut self.statements {
            s.reset_spans();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnDefArg<'a> {
    pub name: Ident<'a>,
}

impl<'a> FnDefArg<'a> {
    pub fn reset_spans(&mut self) {
        self.name.span_reset();
    }
}
