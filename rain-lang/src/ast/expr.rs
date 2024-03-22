use super::{
    function_call::FnCall, if_condition::IfCondition, item::Item, match_expr::Match, Ast,
    ParseError,
};
use crate::{
    error::RainError,
    tokens::{peek_stream::PeekTokenStream, NextTokenSpan, Token, TokenSpan},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr<'a> {
    Item(Item<'a>),
    FnCall(FnCall<'a>),
    BoolLiteral(bool),
    StringLiteral(&'a str),
    IfCondition(IfCondition<'a>),
    Match(Match<'a>),
}

impl<'a> Expr<'a> {
    pub fn parse_stream(stream: &mut PeekTokenStream<'a>) -> Result<Self, RainError> {
        let peeking = stream.peek()?;
        let first_token_span = match peeking.value() {
            NextTokenSpan::Next(token_span) => token_span,
            NextTokenSpan::End(span) => {
                return Err(RainError::new(ParseError::EmptyExpression, *span));
            }
        };
        match first_token_span.token {
            Token::TrueLiteral => {
                peeking.consume();
                Ok(Expr::BoolLiteral(true))
            }
            Token::FalseLiteral => {
                peeking.consume();
                Ok(Expr::BoolLiteral(false))
            }
            Token::DoubleQuoteLiteral(value) => {
                peeking.consume();
                Ok(Expr::StringLiteral(value))
            }
            Token::If => Ok(Expr::IfCondition(IfCondition::parse_stream(stream)?)),
            Token::Match => Ok(Expr::Match(Match::parse_stream(stream)?)),
            Token::Ident(_) => {
                let item = Item::parse_stream(stream)?;
                let peeking = stream.peek()?;
                if let NextTokenSpan::Next(TokenSpan {
                    token: Token::LParen,
                    ..
                }) = peeking.value()
                {
                    Ok(Expr::FnCall(FnCall::parse_stream_item(item, stream)?))
                } else {
                    Ok(Expr::Item(item))
                }
            }
            _ => Err(RainError::new(
                ParseError::UnexpectedTokens,
                first_token_span.span,
            )),
        }
    }
}

impl Ast for Expr<'_> {
    fn reset_spans(&mut self) {
        match self {
            Expr::Item(inner) => inner.reset_spans(),
            Expr::FnCall(inner) => inner.reset_spans(),
            Expr::BoolLiteral(_) => (),
            Expr::StringLiteral(_) => (),
            Expr::IfCondition(inner) => inner.reset_spans(),
            Expr::Match(inner) => inner.reset_spans(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ast::{block::Block, ident::Ident, if_condition::ElseCondition},
        span::Span,
    };

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
        "core.print",
        Expr::Item(Item {
            idents: vec![
                Ident {
                    name: "core",
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
        "core.print(a, b)",
        Expr::FnCall(FnCall {
            item: Item {
                idents: vec![
                    Ident {
                        name: "core",
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
        "core.print(\"hello world\")",
        Expr::FnCall(FnCall {
            item: Item {
                idents: vec![
                    Ident {
                        name: "core",
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

    parse_expr_test!(
        parse_if,
        "if true {}",
        Expr::IfCondition(IfCondition::nosp(
            Expr::BoolLiteral(true),
            Block::nosp(vec![]),
            None,
        ))
    );

    parse_expr_test!(
        parse_if_else,
        "if false {} else {}",
        Expr::IfCondition(IfCondition::nosp(
            Expr::BoolLiteral(false),
            Block::nosp(vec![]),
            Some(ElseCondition::nosp(Block::nosp(vec![])))
        ))
    );
}
