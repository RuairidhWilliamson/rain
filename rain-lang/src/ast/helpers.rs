use crate::{
    error::RainError,
    tokens::{
        peek_stream::{PeekNextTokenSpan, PeekTokenStream},
        NextTokenSpan, TokenKind, TokenSpan,
    },
};

use super::ParseError;

pub trait PeekTokenStreamHelpers<'a> {
    fn expect_parse_next(&mut self, token_kind: TokenKind) -> Result<TokenSpan<'a>, RainError>;
}

impl<'a> PeekTokenStreamHelpers<'a> for PeekTokenStream<'a> {
    fn expect_parse_next(&mut self, token_kind: TokenKind) -> Result<TokenSpan<'a>, RainError> {
        let token = self.parse_next()?.expect_next(token_kind)?;
        token.expect(token_kind)?;
        Ok(token)
    }
}

pub trait PeekNextTokenHelpers<'a> {
    fn expect_not_end(&self, err: ParseError) -> Result<&TokenSpan<'a>, RainError>;
}

impl<'a> PeekNextTokenHelpers<'a> for PeekNextTokenSpan<'a, '_> {
    fn expect_not_end(&self, err: ParseError) -> Result<&TokenSpan<'a>, RainError> {
        match self.value() {
            NextTokenSpan::Next(token) => Ok(token),
            NextTokenSpan::End(span) => Err(RainError::new(err, *span)),
        }
    }
}

pub trait NextTokenSpanHelpers<'a> {
    fn expect_next(self, token_kind: TokenKind) -> Result<TokenSpan<'a>, RainError>;
}

impl<'a> NextTokenSpanHelpers<'a> for NextTokenSpan<'a> {
    fn expect_next(self, token_kind: TokenKind) -> Result<TokenSpan<'a>, RainError> {
        match self {
            NextTokenSpan::Next(token) => Ok(token),
            NextTokenSpan::End(span) => Err(RainError::new(ParseError::Expected(token_kind), span)),
        }
    }
}

pub trait TokenSpanHelpers<'a> {
    fn expect(&self, token_kind: TokenKind) -> Result<(), RainError>;
}

impl<'a> TokenSpanHelpers<'a> for TokenSpan<'a> {
    fn expect(&self, token_kind: TokenKind) -> Result<(), RainError> {
        if TokenKind::from(&self.token) == token_kind {
            Ok(())
        } else {
            Err(RainError::new(ParseError::Expected(token_kind), self.span))
        }
    }
}
