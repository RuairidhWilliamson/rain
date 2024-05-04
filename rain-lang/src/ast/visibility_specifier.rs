use crate::{
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, TokenKind},
};

use super::{helpers::PeekTokenStreamHelpers, Ast};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibilitySpecifier {
    pub_token: Span,
}

impl VisibilitySpecifier {
    pub fn parse_stream(stream: &mut PeekTokenStream) -> Result<Self, RainError> {
        let pub_token = stream.expect_parse_next(TokenKind::Pub)?.span;
        Ok(Self { pub_token })
    }

    pub fn nosp() -> Self {
        Self {
            pub_token: Span::default(),
        }
    }
}

impl Ast for VisibilitySpecifier {
    fn span(&self) -> Span {
        self.pub_token
    }

    fn reset_spans(&mut self) {
        self.pub_token.reset();
    }
}
