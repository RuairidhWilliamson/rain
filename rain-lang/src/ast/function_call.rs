use crate::{
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, Token, TokenKind, TokenSpan},
};

use super::{
    expr::Expr,
    helpers::{NextTokenSpanHelpers, PeekTokenStreamHelpers, TokenSpanHelpers},
    ident::Ident,
    Ast, ParseError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnCall {
    pub expr: Box<Expr>,
    pub lparen_token: Span,
    pub args: Vec<FnCallArg>,
    pub rparen_token: Span,
}

impl FnCall {
    pub fn parse_stream(expr: Expr, stream: &mut PeekTokenStream) -> Result<Self, RainError> {
        let lparen_token = stream.expect_parse_next(TokenKind::LParen)?.span;
        let mut args = Vec::default();
        let rparen_token: TokenSpan;
        loop {
            let peeking = stream.peek()?;
            if peeking
                .value()
                .ref_expect_not_end(ParseError::Expected(TokenKind::RParen))?
                .token
                == Token::RParen
            {
                rparen_token = peeking.consume().expect_next(TokenKind::RParen)?;
                break;
            }
            if peeking
                .value()
                .ref_expect_not_end(ParseError::Expected(TokenKind::Ident))?
                .token
                .kind()
                == TokenKind::Ident
            {
                let peeking = stream.peek_many(2)?;
                if peeking
                    .get(1)
                    .ref_expect_not_end(ParseError::Expected(TokenKind::RParen))?
                    .token
                    == Token::Equals
                {
                    let name = Some(Ident::parse(stream.expect_parse_next(TokenKind::Ident)?)?);
                    stream.expect_parse_next(TokenKind::Equals)?;
                    let value = Expr::parse_stream(stream)?;
                    args.push(FnCallArg { name, value });
                } else {
                    let value = Expr::parse_stream(stream)?;
                    args.push(FnCallArg { name: None, value });
                }
            } else {
                let value = Expr::parse_stream(stream)?;
                args.push(FnCallArg { name: None, value });
            }
            let next_token = stream
                .parse_next()?
                .expect_not_end(ParseError::Expected(TokenKind::RParen))?;
            if next_token.token == Token::Comma {
                continue;
            }
            next_token.expect(TokenKind::RParen)?;
            rparen_token = next_token;
            break;
        }
        let rparen_token = rparen_token.span;
        Ok(Self {
            expr: Box::new(expr),
            lparen_token,
            args,
            rparen_token,
        })
    }

    pub fn nosp(expr: Expr, args: Vec<FnCallArg>) -> Self {
        Self {
            expr: Box::new(expr),
            lparen_token: Span::default(),
            args,
            rparen_token: Span::default(),
        }
    }
}

impl Ast for FnCall {
    fn span(&self) -> Span {
        self.expr.span().combine(self.rparen_token)
    }

    fn reset_spans(&mut self) {
        self.expr.reset_spans();
        self.lparen_token.reset();
        for a in &mut self.args {
            a.reset_spans();
        }
        self.rparen_token.reset();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnCallArg {
    pub name: Option<Ident>,
    pub value: Expr,
}

impl FnCallArg {
    pub fn nosp(name: Option<Ident>, value: Expr) -> Self {
        Self { name, value }
    }
}

impl Ast for FnCallArg {
    fn span(&self) -> Span {
        self.value.span()
    }

    fn reset_spans(&mut self) {
        if let Some(n) = &mut self.name {
            n.reset_spans();
        }
        self.value.reset_spans();
    }
}
