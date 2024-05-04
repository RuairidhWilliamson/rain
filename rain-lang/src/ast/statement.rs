use crate::{
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, TokenKind},
};

use super::{
    declare::Declare, expr::Expr, function_def::FnDef, helpers::NextTokenSpanHelpers,
    return_stmt::Return, visibility_specifier::VisibilitySpecifier, Ast, ParseError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    Expr(Expr),
    LetDeclare(Declare),
    LazyDeclare(Declare),
    FnDef(FnDef),
    Return(Return),
}

impl Statement {
    pub fn parse_stream(stream: &mut PeekTokenStream) -> Result<Self, RainError> {
        let peeking = stream.peek()?;
        let peeking_token = peeking
            .value()
            .ref_expect_not_end(ParseError::ExpectedStmt)?;
        match TokenKind::from(&peeking_token.token) {
            TokenKind::Let => Ok(Self::LetDeclare(Declare::parse_stream_let(None, stream)?)),
            TokenKind::Lazy => Ok(Self::LazyDeclare(Declare::parse_stream_lazy(None, stream)?)),
            TokenKind::Fn => Ok(Self::FnDef(FnDef::parse_stream(None, stream)?)),
            TokenKind::Return => Ok(Self::Return(Return::parse_stream(stream)?)),
            TokenKind::Pub => {
                let visibility = VisibilitySpecifier::parse_stream(stream)?;
                let peeking = stream.peek()?;
                let peeking_token =
                    peeking
                        .value()
                        .ref_expect_not_end(ParseError::ExpectedAny(&[
                            TokenKind::Let,
                            TokenKind::Lazy,
                            TokenKind::Fn,
                        ]))?;
                match TokenKind::from(&peeking_token.token) {
                    TokenKind::Let => Ok(Self::LetDeclare(Declare::parse_stream_let(
                        Some(visibility),
                        stream,
                    )?)),
                    TokenKind::Lazy => Ok(Self::LazyDeclare(Declare::parse_stream_lazy(
                        Some(visibility),
                        stream,
                    )?)),
                    TokenKind::Fn => {
                        Ok(Self::FnDef(FnDef::parse_stream(Some(visibility), stream)?))
                    }
                    _ => Err(RainError::new(
                        ParseError::ExpectedAny(&[TokenKind::Let, TokenKind::Lazy, TokenKind::Fn]),
                        peeking_token.span,
                    )),
                }
            }
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
            Self::FnDef(inner) => inner.span(),
            Self::Return(inner) => inner.span(),
        }
    }

    fn reset_spans(&mut self) {
        match self {
            Self::Expr(inner) => inner.reset_spans(),
            Self::LetDeclare(inner) => inner.reset_spans(),
            Self::LazyDeclare(inner) => inner.reset_spans(),
            Self::FnDef(inner) => inner.reset_spans(),
            Self::Return(inner) => inner.reset_spans(),
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

    parse_statement_test!(
        parse_declare,
        "let a = b",
        Statement::LetDeclare(Declare::nosp(
            None,
            Ident::nosp("a"),
            Expr::Ident(Ident::nosp("b")),
        )),
    );

    parse_statement_test!(
        parse_utf8_declare,
        "let 🌧 = \"rain\"",
        Statement::LetDeclare(Declare::nosp(
            None,
            Ident::nosp("🌧"),
            Expr::StringLiteral(StringLiteral::nosp("rain"))
        )),
    );

    parse_statement_test!(
        parse_fn,
        "fn foo() { true }",
        Statement::FnDef(FnDef::nosp(
            None,
            Ident::nosp("foo"),
            Vec::default(),
            Block::nosp(vec![Statement::Expr(Expr::BoolLiteral(BoolLiteral::nosp(
                true
            )))]),
        )),
    );

    parse_statement_test!(
        parse_pub_fn,
        "pub fn foo() { false }",
        Statement::FnDef(FnDef::nosp(
            Some(VisibilitySpecifier::nosp()),
            Ident::nosp("foo"),
            Vec::default(),
            Block::nosp(vec![Statement::Expr(Expr::BoolLiteral(BoolLiteral::nosp(
                false
            )))]),
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
        Statement::LazyDeclare(Declare::nosp(
            None,
            Ident::nosp("a"),
            Ident::nosp("b").into()
        )),
    );

    parse_statement_test!(
        parse_pub_lazy,
        "pub lazy a = b",
        Statement::LazyDeclare(Declare::nosp(
            Some(VisibilitySpecifier::nosp()),
            Ident::nosp("a"),
            Ident::nosp("b").into()
        )),
    );

    parse_statement_test!(
        parse_pub_let,
        "pub let b = a",
        Statement::LetDeclare(Declare::nosp(
            Some(VisibilitySpecifier::nosp()),
            Ident::nosp("b"),
            Ident::nosp("a").into()
        )),
    );
}
