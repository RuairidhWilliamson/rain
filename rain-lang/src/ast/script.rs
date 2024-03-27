use crate::{error::RainError, span::Span, tokens::peek_stream::PeekTokenStream};

use super::{statement_list::StatementList, Ast};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Script<'a> {
    pub statements: StatementList<'a>,
}

impl<'a> Script<'a> {
    pub fn parse_stream(stream: &mut PeekTokenStream<'a>) -> Result<Self, RainError> {
        let statements = StatementList::parse_stream(stream)?;
        Ok(Self { statements })
    }

    pub fn nosp(statements: StatementList<'a>) -> Self {
        Self { statements }
    }
}

impl Ast for Script<'_> {
    fn span(&self) -> Span {
        todo!()
    }

    fn reset_spans(&mut self) {
        self.statements.reset_spans();
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::{
        declare::Declare, dot::Dot, expr::Expr, function_call::FnCall, ident::Ident,
        statement::Statement, string_literal::StringLiteral,
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
            Script::nosp(StatementList::nosp(vec![
                Statement::Expr(Expr::FnCall(FnCall::nosp(
                    Dot::nosp(Some(Ident::nosp("core").into()), Ident::nosp("print")).into(),
                    vec![StringLiteral::nosp("hello world").into()],
                ))),
                Statement::LetDeclare(Declare::nosp(
                    Ident::nosp("msg"),
                    StringLiteral::nosp("okie").into(),
                )),
                Statement::Expr(Expr::FnCall(FnCall::nosp(
                    Dot::nosp(Some(Ident::nosp("core").into()), Ident::nosp("print")).into(),
                    vec![Ident::nosp("msg").into()],
                ))),
                Statement::Expr(Expr::FnCall(FnCall::nosp(
                    Dot::nosp(Some(Ident::nosp("core").into()), Ident::nosp("print")).into(),
                    vec![StringLiteral::nosp("goodbye").into()],
                )))
            ]))
        );
    }
}
