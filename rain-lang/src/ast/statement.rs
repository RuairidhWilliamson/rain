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
    Declare(Declare<'a>),
    FnDef(FnDef<'a>),
    Return(Return<'a>),
}

impl<'a> Statement<'a> {
    pub fn parse_stream(stream: &mut PeekTokenStream<'a>) -> Result<Self, RainError> {
        let peeking = stream.peek()?;
        let peeking_token = peeking.expect_not_end(ParseError::ExpectedStmt)?;
        match TokenKind::from(&peeking_token.token) {
            TokenKind::Let => Ok(Self::Declare(Declare::parse_stream(stream)?)),
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
            Statement::Declare(inner) => inner.span(),
            Statement::FnDef(inner) => inner.span(),
            Statement::Return(inner) => inner.span(),
        }
    }

    fn reset_spans(&mut self) {
        match self {
            Statement::Expr(inner) => inner.reset_spans(),
            Statement::Declare(inner) => inner.reset_spans(),
            Statement::FnDef(inner) => inner.reset_spans(),
            Statement::Return(inner) => inner.reset_spans(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::{
        block::Block, bool_literal::BoolLiteral, ident::Ident, item::Item,
        string_literal::StringLiteral,
    };

    use super::*;

    #[test]
    fn parse_declare() {
        let source = "let a = b";
        let mut token_stream = PeekTokenStream::new(source);
        let mut stmt = Statement::parse_stream(&mut token_stream).unwrap();
        stmt.reset_spans();
        assert_eq!(
            stmt,
            Statement::Declare(Declare::nosp(
                Ident::nosp("a"),
                Expr::Item(Item::nosp(vec![Ident::nosp("b")]))
            ))
        );
    }

    #[test]
    fn parse_declare_utf8() {
        let source = "let 🌧 = \"rain\"";
        let mut token_stream = PeekTokenStream::new(source);
        let mut stmt = Statement::parse_stream(&mut token_stream).unwrap();
        stmt.reset_spans();
        assert_eq!(
            stmt,
            Statement::Declare(Declare::nosp(
                Ident::nosp("🌧"),
                Expr::StringLiteral(StringLiteral::nosp("rain"))
            ))
        );
    }

    #[test]
    fn parse_fn() {
        let source = "fn foo() { true }";
        let mut token_stream = PeekTokenStream::new(source);
        let mut stmt = Statement::parse_stream(&mut token_stream).unwrap();
        stmt.reset_spans();
        assert_eq!(
            stmt,
            Statement::FnDef(FnDef::nosp(
                Ident::nosp("foo"),
                Vec::default(),
                Block::nosp(vec![Statement::Expr(Expr::BoolLiteral(BoolLiteral::nosp(
                    true
                )))]),
            ))
        );
    }

    #[test]
    fn parse_return() {
        let source = "return b";
        let mut token_stream = PeekTokenStream::new(source);
        let mut stmt = Statement::parse_stream(&mut token_stream).unwrap();
        stmt.reset_spans();
        assert_eq!(
            stmt,
            Statement::Return(Return::nosp(Expr::Item(Item::nosp(vec![Ident::nosp("b")]))))
        )
    }
}
