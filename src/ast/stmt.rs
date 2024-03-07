use crate::{
    error::RainError,
    tokens::{peek_stream::PeekTokenStream, TokenKind},
};

use super::{
    declare::Declare, expr::Expr, fn_def::FnDef, helpers::PeekNextTokenHelpers, ParseError,
};

#[derive(Debug, PartialEq, Eq)]
pub enum Stmt<'a> {
    Expr(Expr<'a>),
    Declare(Declare<'a>),
    FnDef(FnDef<'a>),
}

impl<'a> Stmt<'a> {
    pub fn parse_stream(stream: &mut PeekTokenStream<'a>) -> Result<Self, RainError> {
        let peeking = stream.peek()?;
        let peeking_token = peeking.expect_not_end(ParseError::ExpectedStmt)?;
        match TokenKind::from(&peeking_token.token) {
            TokenKind::Let => Ok(Self::Declare(Declare::parse_stream(stream)?)),
            TokenKind::Fn => Ok(Self::FnDef(FnDef::parse_stream(stream)?)),
            _ => Ok(Self::Expr(Expr::parse_stream(stream)?)),
        }
    }

    pub fn reset_spans(&mut self) {
        match self {
            Stmt::Expr(inner) => inner.reset_spans(),
            Stmt::Declare(inner) => inner.reset_spans(),
            Stmt::FnDef(inner) => inner.reset_spans(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ast::{ident::Ident, item::Item},
        span::Span,
    };

    use super::*;

    #[test]
    fn parse_declare() {
        let source = "let a = b";
        let mut token_stream = PeekTokenStream::new(source);
        let mut stmt = Stmt::parse_stream(&mut token_stream).unwrap();
        stmt.reset_spans();
        assert_eq!(
            stmt,
            Stmt::Declare(Declare {
                name: Ident::nosp("a"),
                value: Expr::Item(Item {
                    idents: vec![Ident {
                        name: "b",
                        span: Span::default()
                    }],
                    span: Span::default()
                })
            })
        );
    }

    #[test]
    fn parse_declare_utf8() {
        let source = "let 🌧 = \"rain\"";
        let mut token_stream = PeekTokenStream::new(source);
        let mut stmt = Stmt::parse_stream(&mut token_stream).unwrap();
        stmt.reset_spans();
        assert_eq!(
            stmt,
            Stmt::Declare(Declare {
                name: Ident::nosp("🌧"),
                value: Expr::StringLiteral("rain")
            })
        );
    }
}
