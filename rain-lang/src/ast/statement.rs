use crate::{
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, TokenKind},
};

use super::{
    expr::Expr, helpers::NextTokenSpanHelpers, let_declare::LetDeclare, return_stmt::Return, Ast,
    ParseError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    Expr(Expr),
    LetDeclare(LetDeclare),
    LazyDeclare(LetDeclare),
    Return(Return),
}

impl Statement {
    pub fn parse_stream(stream: &mut PeekTokenStream) -> Result<Self, RainError> {
        let peeking = stream.peek()?;
        let peeking_token = peeking
            .value()
            .ref_expect_not_end(ParseError::ExpectedStmt)?;
        match TokenKind::from(&peeking_token.token) {
            TokenKind::Let => Ok(Self::LetDeclare(LetDeclare::parse_stream_let(
                None, stream,
            )?)),
            TokenKind::Lazy => Ok(Self::LazyDeclare(LetDeclare::parse_stream_lazy(
                None, stream,
            )?)),
            TokenKind::Return => Ok(Self::Return(Return::parse_stream(stream)?)),
            _ => Ok(Self::Expr(Expr::parse_stream(stream)?)),
        }
    }
}

impl Ast for Statement {
    fn span(&self) -> Span {
        match self {
            Self::Expr(inner) => inner.span(),
            Self::LetDeclare(inner) => inner.span(),
            Self::LazyDeclare(inner) => inner.span(),
            Self::Return(inner) => inner.span(),
        }
    }

    fn reset_spans(&mut self) {
        match self {
            Self::Expr(inner) => inner.reset_spans(),
            Self::LetDeclare(inner) => inner.reset_spans(),
            Self::LazyDeclare(inner) => inner.reset_spans(),
            Self::Return(inner) => inner.reset_spans(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::{ident::Ident, string_literal::StringLiteral};

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

    parse_statement_test!(
        parse_declare,
        "let a = b",
        Statement::LetDeclare(LetDeclare::nosp(
            None,
            Ident::nosp("a"),
            Expr::Ident(Ident::nosp("b")),
        )),
    );

    parse_statement_test!(
        parse_utf8_declare,
        "let 🌧 = \"rain\"",
        Statement::LetDeclare(LetDeclare::nosp(
            None,
            Ident::nosp("🌧"),
            Expr::StringLiteral(StringLiteral::nosp("rain"))
        )),
    );

    parse_statement_test!(
        parse_return,
        "return b",
        Statement::Return(Return::nosp(Expr::Ident(Ident::nosp("b")))),
    );

    parse_statement_test!(
        parse_lazy,
        "lazy a = b",
        Statement::LazyDeclare(LetDeclare::nosp(
            None,
            Ident::nosp("a"),
            Ident::nosp("b").into()
        )),
    );
}
