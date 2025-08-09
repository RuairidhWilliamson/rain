use std::{collections::VecDeque, mem::MaybeUninit};

use crate::{
    ast::error::{ParseError, ParseResult},
    local_span::{ErrorLocalSpan, LocalSpan},
};

use super::{Token, TokenError, TokenLocalSpan, stream::TokenStream};

pub struct PeekTokenStream<'a> {
    stream: TokenStream<'a>,
    peeked: VecDeque<TokenLocalSpan>,
}

impl<'a> From<TokenStream<'a>> for PeekTokenStream<'a> {
    fn from(stream: TokenStream<'a>) -> Self {
        Self {
            stream,
            peeked: Default::default(),
        }
    }
}

impl<'a> PeekTokenStream<'a> {
    pub fn new(source: &'a str) -> Self {
        Self::from(TokenStream::new(source))
    }

    pub fn last_span(&self) -> LocalSpan {
        self.stream.last_span()
    }

    pub fn parse_next(&mut self) -> Result<Option<TokenLocalSpan>, ErrorLocalSpan<TokenError>> {
        if let Some(peeked) = self.peeked.pop_front() {
            return Ok(Some(peeked));
        }
        self.stream.parse_next()
    }

    pub fn peek(&mut self) -> Result<Option<TokenLocalSpan>, ErrorLocalSpan<TokenError>> {
        if self.peeked.is_empty() {
            let Some(tls) = self.stream.parse_next()? else {
                return Ok(None);
            };
            self.peeked.push_back(tls);
        }
        Ok(self.peeked.front().copied())
    }

    /// Consumes zero or more newlines
    pub fn skip_if_newline(&mut self) -> Result<(), ErrorLocalSpan<TokenError>> {
        loop {
            let Some(tls) = self.peek()? else {
                return Ok(());
            };
            match tls.token {
                Token::NewLine => {
                    self.parse_next()?;
                }
                _ => return Ok(()),
            }
        }
    }

    /// Consumes zero or more newlines or comments
    pub fn skip_if_newline_or_comment(&mut self) -> Result<(), ErrorLocalSpan<TokenError>> {
        loop {
            let Some(tls) = self.peek()? else {
                return Ok(());
            };
            match tls.token {
                Token::Comment | Token::NewLine => {
                    self.parse_next()?;
                }
                _ => return Ok(()),
            }
        }
    }

    #[expect(unsafe_code)]
    pub fn peek_many<const N: usize>(
        &mut self,
    ) -> Result<Option<[TokenLocalSpan; N]>, ErrorLocalSpan<TokenError>> {
        while self.peeked.len() < N {
            let Some(tls) = self.stream.parse_next()? else {
                return Ok(None);
            };
            self.peeked.push_back(tls);
        }
        debug_assert_eq!(N, self.peeked.len());
        let mut out = [MaybeUninit::uninit(); N];
        for (i, x) in out.iter_mut().enumerate() {
            let Some(tls) = self.peeked.get(i) else {
                unreachable!("peek_many");
            };
            x.write(*tls);
        }
        // Safety: Safe to assume we have written to all the out
        Ok(Some(out.map(|m| unsafe { m.assume_init() })))
    }
}

// Convenience methods
impl PeekTokenStream<'_> {
    pub fn expect_token(
        &mut self,
        tls: Option<TokenLocalSpan>,
        expect: &'static [Token],
    ) -> ParseResult<TokenLocalSpan> {
        let Some(token) = tls else {
            return Err(self
                .last_span()
                .with_error(ParseError::ExpectedToken(expect)));
        };
        if expect.contains(&token.token) {
            Ok(token)
        } else {
            Err(token.span.with_error(ParseError::ExpectedToken(expect)))
        }
    }

    pub fn expect_parse_next(&mut self, expect: &'static [Token]) -> ParseResult<TokenLocalSpan> {
        let tls = self.parse_next()?;
        self.expect_token(tls, expect)
    }

    pub fn expect_peek(&mut self, expect: &'static [Token]) -> ParseResult<TokenLocalSpan> {
        let tls = self.peek()?;
        self.expect_token(tls, expect)
    }
}
