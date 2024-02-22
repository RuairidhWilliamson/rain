use super::{ParseError, ParseErrorKind};
use crate::{
    span::Span,
    tokens::{Token, TokenSpan},
};

#[derive(Debug, PartialEq, Eq)]
pub enum Expr<'a> {
    Item(Item<'a>),
    FnCall(FnCall<'a>),
    BoolLiteral(bool),
    StringLiteral(&'a str),
}

impl<'a> Expr<'a> {
    pub fn parse(tokens: &[TokenSpan<'a>], span: Span) -> Result<Self, ParseError> {
        match tokens {
            [] => Err(ParseError {
                err: ParseErrorKind::EmptyExpression,
                span,
            }),
            [TokenSpan {
                token: Token::TrueLiteral,
                ..
            }] => Ok(Expr::BoolLiteral(true)),
            [TokenSpan {
                token: Token::FalseLiteral,
                ..
            }] => Ok(Expr::BoolLiteral(false)),
            [TokenSpan {
                token: Token::DoubleQuoteLiteral(value),
                ..
            }] => Ok(Expr::StringLiteral(value)),
            [.., TokenSpan {
                token: Token::RParen,
                ..
            }] => Ok(Self::FnCall(FnCall::parse(tokens, span)?)),
            tokens
                if tokens
                    .iter()
                    .all(|ts| matches!(ts.token, Token::Ident(_) | Token::Dot)) =>
            {
                Ok(Self::Item(Item::parse(tokens)?))
            }
            _ => Err(ParseError {
                err: ParseErrorKind::UnexpectedTokens,
                span,
            }),
        }
    }

    pub fn reset_spans(&mut self) {
        match self {
            Expr::Item(inner) => inner.reset_spans(),
            Expr::FnCall(inner) => inner.reset_spans(),
            Expr::BoolLiteral(_) => (),
            Expr::StringLiteral(_) => (),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Item<'a> {
    pub idents: Vec<&'a str>,
    pub span: Span,
}

impl<'a> Item<'a> {
    fn parse(tokens: &[TokenSpan<'a>]) -> Result<Self, ParseError> {
        let idents = tokens
            .iter()
            .filter_map(|t| match &t.token {
                Token::Ident(ident) => Some(*ident),
                Token::Dot => None,
                token => panic!("unexpected token {token:?}"),
            })
            .collect();
        let span = tokens
            .iter()
            .map(|ts| ts.span)
            .reduce(Span::combine)
            .unwrap();
        Ok(Self { idents, span })
    }

    fn reset_spans(&mut self) {
        self.span.reset();
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct FnCall<'a> {
    pub item: Item<'a>,
    pub args: Vec<Expr<'a>>,
    pub span: Span,
}

impl<'a> FnCall<'a> {
    fn parse(tokens: &[TokenSpan<'a>], span: Span) -> Result<Self, ParseError> {
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
    use crate::{
        span::Span,
        tokens::{TokenError, TokenSpan, TokenStream},
    };

    use super::{Expr, FnCall, Item};

    macro_rules! parse_expr_test {
        ($name:ident, $source:expr, $expected: expr) => {
            #[test]
            fn $name() {
                let token_stream = TokenStream::new($source);
                let tokens: Vec<_> = token_stream.collect::<Result<_, TokenError>>().unwrap();
                let mut expr =
                    Expr::parse(&tokens, TokenSpan::span(&tokens).unwrap_or_default()).unwrap();
                expr.reset_spans();
                assert_eq!(expr, $expected);
            }
        };
    }

    parse_expr_test!(
        parse_single_ident,
        "std",
        Expr::Item(Item {
            idents: vec!["std"],
            span: Span::default(),
        })
    );

    parse_expr_test!(
        parse_item,
        "std.print",
        Expr::Item(Item {
            idents: vec!["std", "print"],
            span: Span::default(),
        })
    );

    parse_expr_test!(
        parse_fn_call,
        "foo()",
        Expr::FnCall(FnCall {
            item: Item {
                idents: vec!["foo"],
                span: Span::default(),
            },
            args: vec![],
            span: Span::default(),
        })
    );

    parse_expr_test!(
        parse_fn_call_arg,
        "foo(a)",
        Expr::FnCall(FnCall {
            item: Item {
                idents: vec!["foo"],
                span: Span::default(),
            },
            args: vec![Expr::Item(Item {
                idents: vec!["a"],
                span: Span::default()
            })],
            span: Span::default(),
        })
    );

    parse_expr_test!(
        parse_fn_call_args,
        "foo(a, b, c)",
        Expr::FnCall(FnCall {
            item: Item {
                idents: vec!["foo"],
                span: Span::default(),
            },
            args: vec![
                Expr::Item(Item {
                    idents: vec!["a"],
                    span: Span::default()
                }),
                Expr::Item(Item {
                    idents: vec!["b"],
                    span: Span::default()
                }),
                Expr::Item(Item {
                    idents: vec!["c"],
                    span: Span::default()
                })
            ],
            span: Span::default(),
        })
    );

    parse_expr_test!(
        parse_print_call,
        "std.print(a, b)",
        Expr::FnCall(FnCall {
            item: Item {
                idents: vec!["std", "print"],
                span: Span::default(),
            },
            args: vec![
                Expr::Item(Item {
                    idents: vec!["a"],
                    span: Span::default()
                }),
                Expr::Item(Item {
                    idents: vec!["b"],
                    span: Span::default()
                }),
            ],
            span: Span::default(),
        })
    );

    parse_expr_test!(
        parse_print_hello_world,
        "std.print(\"hello world\")",
        Expr::FnCall(FnCall {
            item: Item {
                idents: vec!["std", "print"],
                span: Span::default(),
            },
            args: vec![Expr::StringLiteral("hello world")],
            span: Span::default(),
        })
    );
}
