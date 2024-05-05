use crate::{
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, TokenKind},
};

use super::{
    function_def::FnDef, helpers::NextTokenSpanHelpers, let_declare::LetDeclare,
    visibility_specifier::VisibilitySpecifier, Ast, ParseError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Declaration {
    LetDeclare(LetDeclare),
    LazyDeclare(LetDeclare),
    FnDeclare(FnDef),
}

impl Declaration {
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
            TokenKind::Fn => Ok(Self::FnDeclare(FnDef::parse_stream(None, stream)?)),
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
                    TokenKind::Let => Ok(Self::LetDeclare(LetDeclare::parse_stream_let(
                        Some(visibility),
                        stream,
                    )?)),
                    TokenKind::Lazy => Ok(Self::LazyDeclare(LetDeclare::parse_stream_lazy(
                        Some(visibility),
                        stream,
                    )?)),
                    TokenKind::Fn => Ok(Self::FnDeclare(FnDef::parse_stream(
                        Some(visibility),
                        stream,
                    )?)),
                    _ => Err(RainError::new(
                        ParseError::ExpectedAny(&[TokenKind::Let, TokenKind::Lazy, TokenKind::Fn]),
                        peeking_token.span,
                    )),
                }
            }
            _ => Err(RainError::new(
                ParseError::ExpectedAny(&[
                    TokenKind::Let,
                    TokenKind::Lazy,
                    TokenKind::Pub,
                    TokenKind::Fn,
                ]),
                peeking_token.span,
            )),
        }
    }

    pub fn name(&self) -> String {
        match self {
            Self::LetDeclare(inner) => inner.name(),
            Self::LazyDeclare(inner) => inner.name(),
            Self::FnDeclare(inner) => inner.name(),
        }
    }
}

impl Ast for Declaration {
    fn span(&self) -> Span {
        match self {
            Self::LetDeclare(inner) => inner.span(),
            Self::LazyDeclare(inner) => inner.span(),
            Self::FnDeclare(inner) => inner.span(),
        }
    }

    fn reset_spans(&mut self) {
        match self {
            Self::LetDeclare(inner) => inner.reset_spans(),
            Self::LazyDeclare(inner) => inner.reset_spans(),
            Self::FnDeclare(inner) => inner.reset_spans(),
        }
    }
}
