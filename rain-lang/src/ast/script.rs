use crate::{
    error::RainError,
    tokens::{peek_stream::PeekTokenStream, NextTokenSpan, Token},
};

use super::stmt::Stmt;

#[derive(Debug, PartialEq, Eq)]
pub struct Script<'a> {
    pub statements: Vec<Stmt<'a>>,
}

impl<'a> Script<'a> {
    pub fn parse_stream(stream: &mut PeekTokenStream<'a>) -> Result<Self, RainError> {
        let mut statements = Vec::new();
        loop {
            let peeking = stream.peek()?;
            let NextTokenSpan::Next(token) = peeking.value() else {
                break;
            };
            if token.token == Token::NewLine {
                peeking.consume();
                continue;
            }
            statements.push(Stmt::parse_stream(stream)?);
        }
        Ok(Self { statements })
    }

    pub fn reset_spans(&mut self) {
        for s in &mut self.statements {
            s.reset_spans();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ast::{declare::Declare, expr::Expr, fn_call::FnCall, ident::Ident, item::Item},
        span::Span,
    };

    use super::*;

    #[test]
    fn parse_script() {
        let source = "core.print(\"hello world\")
        let msg = \"okie\"
        core.print(msg)
        core.print(\"goodbye\")
        ";
        let mut token_stream = PeekTokenStream::new(source);
        let mut script = Script::parse_stream(&mut token_stream).unwrap();
        script.reset_spans();
        assert_eq!(
            script,
            Script {
                statements: vec![
                    Stmt::Expr(Expr::FnCall(FnCall {
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
                    })),
                    Stmt::Declare(Declare {
                        name: Ident::nosp("msg"),
                        value: Expr::StringLiteral("okie")
                    }),
                    Stmt::Expr(Expr::FnCall(FnCall {
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
                        args: vec![Expr::Item(Item {
                            idents: vec![Ident {
                                name: "msg",
                                span: Span::default()
                            }],
                            span: Span::default(),
                        })],
                        span: Span::default(),
                    })),
                    Stmt::Expr(Expr::FnCall(FnCall {
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
                        args: vec![Expr::StringLiteral("goodbye"),],
                        span: Span::default(),
                    }))
                ]
            }
        );
    }
}
