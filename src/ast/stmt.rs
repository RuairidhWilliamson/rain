use crate::{
    error::RainError,
    tokens::{Token, TokenSpan},
};

use super::{declare::Declare, expr::Expr, fn_def::FnDef};

#[derive(Debug, PartialEq, Eq)]
pub enum Stmt<'a> {
    Expr(Expr<'a>),
    Declare(Declare<'a>),
    FnDef(FnDef<'a>),
}

impl<'a> Stmt<'a> {
    pub fn parse(tokens: &[TokenSpan<'a>]) -> Result<Self, RainError> {
        match tokens {
            [] => panic!("empty statement"),
            [TokenSpan {
                token: Token::Let, ..
            }, ..] => Ok(Self::Declare(Declare::parse(tokens)?)),
            [TokenSpan {
                token: Token::Fn, ..
            }, ..] => Ok(Self::FnDef(FnDef::parse(tokens)?)),
            _ => Ok(Self::Expr(Expr::parse(
                tokens,
                TokenSpan::span(tokens).unwrap(),
            )?)),
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
        tokens::{TokenError, TokenStream},
    };

    use super::*;

    #[test]
    fn parse_declare() {
        let source = "let a = b";
        let token_stream = TokenStream::new(source);
        let tokens: Vec<_> = token_stream.collect::<Result<_, TokenError>>().unwrap();
        let mut stmt = Stmt::parse(&tokens).unwrap();
        stmt.reset_spans();
        assert_eq!(
            stmt,
            Stmt::Declare(Declare {
                name: "a",
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
        let token_stream = TokenStream::new(source);
        let tokens: Vec<_> = token_stream.collect::<Result<_, TokenError>>().unwrap();
        let mut stmt = Stmt::parse(&tokens).unwrap();
        stmt.reset_spans();
        assert_eq!(
            stmt,
            Stmt::Declare(Declare {
                name: "🌧",
                value: Expr::StringLiteral("rain")
            })
        );
    }
}
