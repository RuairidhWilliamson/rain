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
    item::Item,
    Ast, ParseError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnCall<'a> {
    pub item: Item<'a>,
    pub args: Vec<Expr<'a>>,
    pub span: Span,
}

impl<'a> FnCall<'a> {
    pub fn parse_stream(stream: &mut PeekTokenStream<'a>) -> Result<Self, RainError> {
        let item = Item::parse_stream(stream)?;
        Self::parse_stream_item(item, stream)
    }

    pub fn parse_stream_item(
        item: Item<'a>,
        stream: &mut PeekTokenStream<'a>,
    ) -> Result<Self, RainError> {
        stream.expect_parse_next(TokenKind::LParen)?;
        let mut args = Vec::default();
        let rparen_token: TokenSpan<'a>;
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
            let next_token = stream.parse_next()?.expect_next(TokenKind::RParen)?;
            if next_token.token == Token::Comma {
                continue;
            }
            next_token.expect(TokenKind::RParen)?;
            rparen_token = next_token;
            break;
        }
        let span = item.span.combine(rparen_token.span);
        Ok(Self { item, args, span })
    }

    pub fn nosp(item: Item<'a>, args: Vec<Expr<'a>>) -> Self {
        Self {
            item,
            args,
            span: Span::default(),
        }
    }
}

impl Ast for FnCall<'_> {
    fn reset_spans(&mut self) {
        self.item.reset_spans();
        for a in &mut self.args {
            a.reset_spans();
        }
        self.span.reset();
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::ident::Ident;

    use super::*;

    fn parse_fn_call(source: &str) -> Result<FnCall, RainError> {
        let mut stream = PeekTokenStream::new(source);
        let mut fn_call = super::FnCall::parse_stream(&mut stream)?;
        fn_call.reset_spans();
        Ok(fn_call)
    }

    #[test]
    fn parse_no_args() -> Result<(), RainError> {
        let fn_call = parse_fn_call("foo()")?;
        assert_eq!(
            fn_call,
            FnCall::nosp(Item::nosp(vec![Ident::nosp("foo")]), vec![])
        );
        Ok(())
    }

    #[test]
    fn parse_one_arg() -> Result<(), RainError> {
        let fn_call = parse_fn_call("foo(bar)")?;
        assert_eq!(
            fn_call,
            FnCall::nosp(
                Item::nosp(vec![Ident::nosp("foo")]),
                vec![Expr::Item(Item::nosp(vec![Ident::nosp("bar")]))],
            )
        );
        Ok(())
    }
}
