use crate::{
    error::RainError,
    tokens::{peek_stream::PeekTokenStream, NextTokenSpan, TokenKind, TokenSpan},
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

pub trait NextTokenSpanHelpers<'a> {
    fn ref_expect_not_end<'b>(&'b self, err: ParseError) -> Result<&'b TokenSpan<'a>, RainError>;
    fn expect_not_end(self, err: ParseError) -> Result<TokenSpan<'a>, RainError>;
    fn expect_next(self, token_kind: TokenKind) -> Result<TokenSpan<'a>, RainError>;
    #[allow(dead_code)]
    fn expect_next_any(self, token_kinds: &'static [TokenKind])
        -> Result<TokenSpan<'a>, RainError>;
}

impl<'a> NextTokenSpanHelpers<'a> for NextTokenSpan<'a> {
    fn ref_expect_not_end<'b>(&'b self, err: ParseError) -> Result<&'b TokenSpan<'a>, RainError> {
        match self {
            NextTokenSpan::Next(token) => Ok(token),
            NextTokenSpan::End(span) => Err(RainError::new(err, *span)),
        }
    }

    fn expect_not_end(self, err: ParseError) -> Result<TokenSpan<'a>, RainError> {
        match self {
            NextTokenSpan::Next(token) => Ok(token),
            NextTokenSpan::End(span) => Err(RainError::new(err, span)),
        }
    }

    fn expect_next(self, token_kind: TokenKind) -> Result<TokenSpan<'a>, RainError> {
        let token = self.expect_not_end(ParseError::Expected(token_kind))?;
        if token.token.kind() == token_kind {
            Ok(token)
        } else {
            Err(RainError::new(ParseError::Expected(token_kind), token.span))
        }
    }

    fn expect_next_any(
        self,
        token_kinds: &'static [TokenKind],
    ) -> Result<TokenSpan<'a>, RainError> {
        let token = self.expect_not_end(ParseError::ExpectedAny(token_kinds))?;
        if token_kinds.contains(&token.token.kind()) {
            Ok(token)
        } else {
            Err(RainError::new(
                ParseError::ExpectedAny(token_kinds),
                token.span,
            ))
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
