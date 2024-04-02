use crate::{
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, Token, TokenKind, TokenSpan},
};

use super::{
    expr::Expr,
    helpers::{
        NextTokenSpanHelpers, PeekNextTokenHelpers, PeekTokenStreamHelpers, TokenSpanHelpers,
    },
    Ast, ParseError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnCall {
    pub expr: Box<Expr>,
    pub lparen_token: Span,
    pub args: Vec<Expr>,
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
                .expect_not_end(ParseError::Expected(TokenKind::RParen))?
                .token
                == Token::RParen
            {
                rparen_token = peeking.consume().expect_next(TokenKind::RParen)?;
                break;
            }
            let expr = Expr::parse_stream(stream)?;
            args.push(expr);
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

    pub fn nosp(expr: Expr, args: Vec<Expr>) -> Self {
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

// #[cfg(test)]
// mod tests {
//     use crate::ast::ident::Ident;

//     use super::*;

//     fn parse_fn_call(source: &str) -> Result<FnCall, RainError> {
//         let mut stream = PeekTokenStream::new(source);
//         let mut fn_call = super::FnCall::parse_stream(&mut stream)?;
//         fn_call.reset_spans();
//         Ok(fn_call)
//     }

//     #[test]
//     fn parse_no_args() -> Result<(), RainError> {
//         let fn_call = parse_fn_call("foo()")?;
//         assert_eq!(
//             fn_call,
//             FnCall::nosp(Item::nosp(vec![Ident::nosp("foo")]), vec![])
//         );
//         Ok(())
//     }

//     #[test]
//     fn parse_one_arg() -> Result<(), RainError> {
//         let fn_call = parse_fn_call("foo(bar)")?;
//         assert_eq!(
//             fn_call,
//             FnCall::nosp(
//                 Item::nosp(vec![Ident::nosp("foo")]),
//                 vec![Expr::Item(Item::nosp(vec![Ident::nosp("bar")]))],
//             )
//         );
//         Ok(())
//     }
// }
