use super::{fn_call::FnCall, item::Item, ParseError};
use crate::{
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, NextTokenSpan, Token, TokenKind, TokenSpan},
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

    pub fn parse_stream(stream: &mut PeekTokenStream<'a>) -> Result<Self, RainError> {
        let first_token_span = match stream.parse_next()? {
            NextTokenSpan::Next(token_span) => token_span,
            NextTokenSpan::End(span) => {
                return Err(RainError::new(ParseError::EmptyExpression, span));
            }
        };

        todo!()
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
    use crate::ast::ident::Ident;

    use super::*;

    macro_rules! parse_expr_test {
        ($name:ident, $source:expr, $expected: expr) => {
            #[test]
            fn $name() -> Result<(), RainError> {
                let mut token_stream = PeekTokenStream::new($source);
                let mut expr = Expr::parse_stream(&mut token_stream)?;
                expr.reset_spans();
                assert_eq!(expr, $expected);
                Ok(())
            }
        };
    }

    parse_expr_test!(
        parse_single_ident,
        "std",
        Expr::Item(Item::nosp(vec![Ident::nosp("std")]))
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
