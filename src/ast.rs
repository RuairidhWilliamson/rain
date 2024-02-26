pub mod expr;
pub mod stmt;

use crate::{
    error::RainError,
    tokens::{Token, TokenSpan},
};

use self::stmt::Stmt;

#[derive(Debug, PartialEq, Eq)]
pub struct Script<'a> {
    pub statements: Vec<Stmt<'a>>,
}

impl<'a> Script<'a> {
    pub fn parse(tokens: &[TokenSpan<'a>]) -> Result<Self, RainError> {
        let statements = tokens
            .split(|ts| ts.token == Token::NewLine)
            .filter_map(|tss| {
                if tss.is_empty() {
                    None
                } else {
                    Some(Stmt::parse(tss))
                }
            })
            .collect::<Result<Vec<Stmt>, RainError>>()?;
        Ok(Self { statements })
    }

    pub fn reset_spans(&mut self) {
        for s in &mut self.statements {
            s.reset_spans();
        }
    }
}

#[derive(Debug)]
pub enum ParseError {
    EmptyExpression,
    UnexpectedTokens,
    ExpectedAssignToken,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        span::Span,
        tokens::{TokenError, TokenStream},
    };

    use super::{
        expr::{Expr, FnCall, Item},
        stmt::{Declare, Stmt},
        Script,
    };

    #[test]
    fn parse_script() {
        let source = "std.print(\"hello world\")
        let msg = \"okie\"
        std.print(msg)
        std.print(\"goodbye\")
        ";
        let token_stream = TokenStream::new(source);
        let tokens: Vec<_> = token_stream.collect::<Result<_, TokenError>>().unwrap();
        let mut script = Script::parse(&tokens).unwrap();
        script.reset_spans();
        assert_eq!(
            script,
            Script {
                statements: vec![
                    Stmt::Expr(Expr::FnCall(FnCall {
                        item: Item {
                            idents: vec!["std", "print"],
                            span: Span::default(),
                        },
                        args: vec![Expr::StringLiteral("hello world")],
                        span: Span::default(),
                    })),
                    Stmt::Declare(Declare {
                        name: "msg",
                        value: Expr::StringLiteral("okie")
                    }),
                    Stmt::Expr(Expr::FnCall(FnCall {
                        item: Item {
                            idents: vec!["std", "print"],
                            span: Span::default(),
                        },
                        args: vec![Expr::Item(Item {
                            idents: vec!["msg"],
                            span: Span::default(),
                        })],
                        span: Span::default(),
                    })),
                    Stmt::Expr(Expr::FnCall(FnCall {
                        item: Item {
                            idents: vec!["std", "print"],
                            span: Span::default(),
                        },
                        args: vec![Expr::StringLiteral("goodbye"),],
                        span: Span::default(),
                    }))
                ]
            }
        )
    }
}
