use super::{fn_call::FnCall, item::Item, ParseError};
use crate::{
    error::RainError,
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
    pub fn parse(tokens: &[TokenSpan<'a>], span: Span) -> Result<Self, RainError> {
        match tokens {
            [] => Err(RainError::new(ParseError::EmptyExpression, span)),
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
            _ => Err(RainError::new(ParseError::UnexpectedTokens, span)),
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

#[cfg(test)]
mod tests {
    use crate::{
        ast::{fn_call::FnCall, ident::Ident, item::Item},
        span::Span,
        tokens::{TokenError, TokenSpan, TokenStream},
    };

    use super::Expr;

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
            idents: vec![Ident {
                name: "std",
                span: Span::default()
            }],
            span: Span::default(),
        })
    );

    parse_expr_test!(
        parse_item,
        "std.print",
        Expr::Item(Item {
            idents: vec![
                Ident {
                    name: "std",
                    span: Span::default()
                },
                Ident {
                    name: "print",
                    span: Span::default()
                }
            ],
            span: Span::default(),
        })
    );

    parse_expr_test!(
        parse_fn_call,
        "foo()",
        Expr::FnCall(FnCall {
            item: Item {
                idents: vec![Ident {
                    name: "foo",
                    span: Span::default()
                }],
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
                idents: vec![Ident {
                    name: "foo",
                    span: Span::default()
                }],
                span: Span::default(),
            },
            args: vec![Expr::Item(Item {
                idents: vec![Ident {
                    name: "a",
                    span: Span::default()
                }],
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
                idents: vec![Ident {
                    name: "foo",
                    span: Span::default()
                }],
                span: Span::default(),
            },
            args: vec![
                Expr::Item(Item {
                    idents: vec![Ident {
                        name: "a",
                        span: Span::default()
                    }],
                    span: Span::default()
                }),
                Expr::Item(Item {
                    idents: vec![Ident {
                        name: "b",
                        span: Span::default()
                    }],
                    span: Span::default()
                }),
                Expr::Item(Item {
                    idents: vec![Ident {
                        name: "c",
                        span: Span::default()
                    }],
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
                idents: vec![
                    Ident {
                        name: "std",
                        span: Span::default()
                    },
                    Ident {
                        name: "print",
                        span: Span::default()
                    }
                ],
                span: Span::default(),
            },
            args: vec![
                Expr::Item(Item {
                    idents: vec![Ident {
                        name: "a",
                        span: Span::default()
                    }],
                    span: Span::default()
                }),
                Expr::Item(Item {
                    idents: vec![Ident {
                        name: "b",
                        span: Span::default()
                    }],
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
                idents: vec![
                    Ident {
                        name: "std",
                        span: Span::default()
                    },
                    Ident {
                        name: "print",
                        span: Span::default()
                    }
                ],
                span: Span::default(),
            },
            args: vec![Expr::StringLiteral("hello world")],
            span: Span::default(),
        })
    );
}
