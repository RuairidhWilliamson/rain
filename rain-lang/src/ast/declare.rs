use crate::{
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, TokenKind},
};

use super::{
    expr::Expr,
    helpers::{NextTokenSpanHelpers, PeekTokenStreamHelpers},
    ident::Ident,
    visibility_specifier::VisibilitySpecifier,
    Ast,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declare {
    pub visibility: Option<VisibilitySpecifier>,
    pub token: Span,
    pub name: Ident,
    pub equals_token: Span,
    pub value: Expr,
}

impl Declare {
    pub fn parse_stream_let(
        visibility: Option<VisibilitySpecifier>,
        stream: &mut PeekTokenStream,
    ) -> Result<Self, RainError> {
        let token = stream.parse_next()?.expect_next(TokenKind::Let)?;
        Self::parse_stream(visibility, token.span, stream)
    }

    pub fn parse_stream_lazy(
        visibility: Option<VisibilitySpecifier>,
        stream: &mut PeekTokenStream,
    ) -> Result<Self, RainError> {
        let token = stream.parse_next()?.expect_next(TokenKind::Lazy)?;
        Self::parse_stream(visibility, token.span, stream)
    }

    fn parse_stream(
        visibility: Option<VisibilitySpecifier>,
        token: Span,
        stream: &mut PeekTokenStream,
    ) -> Result<Self, RainError> {
        let ident_token = stream.expect_parse_next(TokenKind::Ident)?;
        let name = Ident::parse(ident_token)?;
        let equals_token = stream.expect_parse_next(TokenKind::Equals)?.span;
        let value = Expr::parse_stream(stream)?;
        Ok(Self {
            visibility,
            token,
            name,
            equals_token,
            value,
        })
    }

    pub fn nosp(visibility: Option<VisibilitySpecifier>, name: Ident, value: Expr) -> Self {
        Self {
            visibility,
            token: Span::default(),
            name,
            equals_token: Span::default(),
            value,
        }
    }
}

impl Ast for Declare {
    fn span(&self) -> Span {
        self.token.combine(self.value.span())
    }

    fn reset_spans(&mut self) {
        for v in &mut self.visibility {
            v.reset_spans();
        }
        self.token.reset();
        self.name.reset_spans();
        self.equals_token.reset();
        self.value.reset_spans();
    }
}
