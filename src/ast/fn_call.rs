use crate::{
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, Token, TokenSpan},
};

use super::{expr::Expr, item::Item};

#[derive(Debug, PartialEq, Eq)]
pub struct FnCall<'a> {
    pub item: Item<'a>,
    pub args: Vec<Expr<'a>>,
    pub span: Span,
}

impl<'a> FnCall<'a> {
    pub fn parse(tokens: &[TokenSpan<'a>], span: Span) -> Result<Self, RainError> {
        let (rparen, tokens) = tokens.split_last().unwrap();
        assert_eq!(rparen.token, Token::RParen);
        let Some((lparen_index, _)) = tokens
            .iter()
            .enumerate()
            .find(|(_, ts)| ts.token == Token::LParen)
        else {
            panic!("missing lparen")
        };
        let (item_tokens, args) = tokens.split_at(lparen_index);
        let item = Item::parse(item_tokens)?;
        let (lparen, args) = args.split_first().unwrap();
        assert_eq!(lparen.token, Token::LParen);
        let args = if args.is_empty() {
            vec![]
        } else {
            args.split(|ts| ts.token == Token::Comma)
                .map(|tokens| Expr::parse(tokens, span).unwrap())
                .collect()
        };
        let span = item.span.combine(rparen.span);
        Ok(Self { item, args, span })
    }

    pub fn parse_stream(stream: &mut PeekTokenStream<'a>) -> Result<Self, RainError> {
        let item = Item::parse_stream(stream)?;
        let span = Span::default();
        Ok(Self {
            item,
            args: Vec::default(),
            span,
        })
    }

    pub fn nosp(item: Item<'a>, args: Vec<Expr<'a>>) -> Self {
        Self {
            item,
            args,
            span: Span::default(),
        }
    }

    pub fn reset_spans(&mut self) {
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
