use crate::{
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, TokenKind},
};

use super::{
    declare::Declare, expr::Expr, function_def::FnDef, helpers::PeekNextTokenHelpers,
    return_stmt::Return, Ast, ParseError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement<'a> {
    Expr(Expr<'a>),
    LetDeclare(Declare<'a>),
    LazyDeclare(Declare<'a>),
    FnDef(FnDef<'a>),
    Return(Return<'a>),
}

impl<'a> Statement<'a> {
    pub fn parse_stream(stream: &mut PeekTokenStream<'a>) -> Result<Self, RainError> {
        let peeking = stream.peek()?;
        let peeking_token = peeking.expect_not_end(ParseError::ExpectedStmt)?;
        match TokenKind::from(&peeking_token.token) {
            TokenKind::Let => Ok(Self::LetDeclare(Declare::parse_stream_let(stream)?)),
            TokenKind::Lazy => Ok(Self::LazyDeclare(Declare::parse_stream_lazy(stream)?)),
            TokenKind::Fn => Ok(Self::FnDef(FnDef::parse_stream(stream)?)),
            TokenKind::Return => Ok(Self::Return(Return::parse_stream(stream)?)),
            _ => Ok(Self::Expr(Expr::parse_stream(stream)?)),
        }
    }
}

impl Ast for Statement<'_> {
    fn span(&self) -> Span {
        match self {
            Statement::Expr(inner) => inner.span(),
            Statement::LetDeclare(inner) => inner.span(),
            Statement::LazyDeclare(inner) => inner.span(),
            Statement::FnDef(inner) => inner.span(),
            Statement::Return(inner) => inner.span(),
        }
    }

    fn reset_spans(&mut self) {
        match self {
            Statement::Expr(inner) => inner.reset_spans(),
            Statement::LetDeclare(inner) => inner.reset_spans(),
            Statement::LazyDeclare(inner) => inner.reset_spans(),
            Statement::FnDef(inner) => inner.reset_spans(),
            Statement::Return(inner) => inner.reset_spans(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::{
        block::Block, bool_literal::BoolLiteral, ident::Ident, string_literal::StringLiteral,
    };

    use super::*;

    macro_rules! parse_statement_test {
        ($name:ident, $source:expr, $expected:expr) => {
            #[test]
            fn $name() -> Result<(), RainError> {
                let mut token_stream = PeekTokenStream::new($source);
                let mut stmt = Statement::parse_stream(&mut token_stream)?;
                stmt.reset_spans();
                assert_eq!(stmt, $expected);
                Ok(())
            }
        };
        ($name:ident, $source:expr, $expected:expr,) => {
            parse_statement_test!($name, $source, $expected);
        };
    }

    // parse_statement_test!(
    //     parse_declare,
    //     "let a = b",
    //     Statement::LetDeclare(Declare::nosp(
    //         Ident::nosp("a"),
    //         Expr::Item(Item::nosp(vec![Ident::nosp("b")]))
    //     )),
    // );

    parse_statement_test!(
        parse_utf8_declare,
        "let 🌧 = \"rain\"",
        Statement::LetDeclare(Declare::nosp(
            Ident::nosp("🌧"),
            Expr::StringLiteral(StringLiteral::nosp("rain"))
        )),
    );

    parse_statement_test!(
        parse_fn,
        "fn foo() { true }",
        Statement::FnDef(FnDef::nosp(
            Ident::nosp("foo"),
            Vec::default(),
            Block::nosp(vec![Statement::Expr(Expr::BoolLiteral(BoolLiteral::nosp(
                true
            )))]),
        )),
    );

    // parse_statement_test!(
    //     parse_return,
    //     "return b",
    //     Statement::Return(Return::nosp(Expr::Item(Item::nosp(vec![Ident::nosp("b")])))),
    // );

    // parse_statement_test!(
    //     parse_lazy,
    //     "lazy a = b",
    //     Statement::LazyDeclare(Declare::nosp(
    //         Ident::nosp("a"),
    //         Expr::Item(Item::nosp(vec![Ident::nosp("b")])),
    //     )),
    // );
}
