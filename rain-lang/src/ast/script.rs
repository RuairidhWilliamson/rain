use crate::{error::RainError, tokens::peek_stream::PeekTokenStream};

use super::{statement_list::StatementList, Ast};

#[derive(Debug, PartialEq, Eq)]
pub struct Script<'a> {
    pub statements: StatementList<'a>,
}

impl<'a> Script<'a> {
    pub fn parse_stream(stream: &mut PeekTokenStream<'a>) -> Result<Self, RainError> {
        let statements = StatementList::parse_stream(stream)?;
        Ok(Self { statements })
    }
}

impl Ast for Script<'_> {
    fn reset_spans(&mut self) {
        self.statements.reset_spans();
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ast::{
            declare::Declare, expr::Expr, fn_call::FnCall, ident::Ident, item::Item, stmt::Stmt,
        },
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
                statements: StatementList::nosp(vec![
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
                ])
            }
        );
    }
}
