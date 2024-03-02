use crate::{
    error::RainError,
    span::Span,
    tokens::{Token, TokenSpan},
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

    pub fn reset_spans(&mut self) {
        self.item.reset_spans();
        for a in &mut self.args {
            a.reset_spans();
        }
        self.span.reset();
    }
}
