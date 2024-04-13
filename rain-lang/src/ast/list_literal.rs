use crate::{
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, Token, TokenKind, TokenSpan},
};

use super::{
    expr::Expr,
    helpers::{NextTokenSpanHelpers, PeekTokenStreamHelpers, TokenSpanHelpers},
    Ast, ParseError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListLiteral {
    pub lbracket_token: Span,
    pub elements: Vec<Expr>,
    pub rbracket_token: Span,
}

impl ListLiteral {
    pub fn parse_stream(stream: &mut PeekTokenStream) -> Result<Self, RainError> {
        let lbracket_token = stream.expect_parse_next(TokenKind::LBracket)?.span;
        let mut elements = Vec::default();
        let rbracket_token: TokenSpan;
        loop {
            let peeking = stream.peek()?;
            if peeking
                .value()
                .ref_expect_not_end(ParseError::Expected(TokenKind::RBracket))?
                .token
                == Token::RBracket
            {
                rbracket_token = peeking.consume().expect_next(TokenKind::RBracket)?;
                break;
            }
            let expr = Expr::parse_stream(stream)?;
            elements.push(expr);
            let next_token = stream
                .parse_next()?
                .expect_not_end(ParseError::Expected(TokenKind::RBracket))?;
            if next_token.token == Token::Comma {
                continue;
            }
            next_token.expect(TokenKind::RBracket)?;
            rbracket_token = next_token;
            break;
        }

        let rbracket_token = rbracket_token.span;
        Ok(Self {
            lbracket_token,
            elements,
            rbracket_token,
        })
    }

    pub fn nosp(elements: Vec<Expr>) -> Self {
        Self {
            lbracket_token: Span::default(),
            elements,
            rbracket_token: Span::default(),
        }
    }
}

impl Ast for ListLiteral {
    fn span(&self) -> Span {
        self.lbracket_token.combine(self.rbracket_token)
    }

    fn reset_spans(&mut self) {
        self.lbracket_token.reset();
        for e in &mut self.elements {
            e.reset_spans();
        }
        self.rbracket_token.reset();
    }
}
