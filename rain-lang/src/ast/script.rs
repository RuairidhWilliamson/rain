use crate::{
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, NextTokenSpan, TokenKind},
};

use super::{declaration::Declaration, Ast};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Script {
    pub declarations: Vec<Declaration>,
}

impl Script {
    pub fn parse_stream(stream: &mut PeekTokenStream) -> Result<Self, RainError> {
        let mut declarations = Vec::new();
        loop {
            let peeking = stream.peek()?;
            let NextTokenSpan::Next(token) = peeking.value() else {
                break;
            };
            match TokenKind::from(&token.token) {
                TokenKind::NewLine => {
                    peeking.consume();
                    continue;
                }
                _ => {
                    declarations.push(Declaration::parse_stream(stream)?);
                }
            }
        }
        Ok(Self { declarations })
    }

    pub fn nosp(declarations: Vec<Declaration>) -> Self {
        Self { declarations }
    }
}

impl Ast for Script {
    fn span(&self) -> Span {
        todo!("script span")
    }

    fn reset_spans(&mut self) {
        for d in &mut self.declarations {
            d.reset_spans();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::{
        block::Block,
        dot::Dot,
        expr::Expr,
        function_call::{FnCall, FnCallArg},
        function_def::FnDef,
        ident::Ident,
        let_declare::LetDeclare,
        statement::Statement,
        string_literal::StringLiteral,
    };

    use super::*;

    #[test]
    fn parse_script() {
        let source = "fn main() {
        core.print(\"hello world\")
        let msg = \"okie\"
        core.print(msg)
        core.print(\"goodbye\")
        }
        ";
        let mut token_stream = PeekTokenStream::new(source);
        let mut script = Script::parse_stream(&mut token_stream).unwrap();
        script.reset_spans();
        assert_eq!(
            script,
            Script::nosp(vec![Declaration::FnDeclare(FnDef::nosp(
                None,
                Ident::nosp("main"),
                vec![],
                Block::nosp(vec![
                    Statement::Expr(Expr::FnCall(FnCall::nosp(
                        Dot::nosp(Some(Ident::nosp("core").into()), Ident::nosp("print")).into(),
                        vec![FnCallArg::nosp(
                            None,
                            StringLiteral::nosp("hello world").into()
                        )],
                    ))),
                    Statement::LetDeclare(LetDeclare::nosp(
                        None,
                        Ident::nosp("msg"),
                        StringLiteral::nosp("okie").into(),
                    )),
                    Statement::Expr(Expr::FnCall(FnCall::nosp(
                        Dot::nosp(Some(Ident::nosp("core").into()), Ident::nosp("print")).into(),
                        vec![FnCallArg::nosp(None, Ident::nosp("msg").into())],
                    ))),
                    Statement::Expr(Expr::FnCall(FnCall::nosp(
                        Dot::nosp(Some(Ident::nosp("core").into()), Ident::nosp("print")).into(),
                        vec![FnCallArg::nosp(None, StringLiteral::nosp("goodbye").into())],
                    )))
                ])
            ))])
        );
    }
}
