use std::{collections::VecDeque, mem::MaybeUninit};

use crate::error::ErrorLocalSpan;

use super::{stream::TokenStream, TokenError, TokenLocalSpan};

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
                unreachable!();
            };
            x.write(*tls);
        }
        // Safety: Safe to assume we have written to all the out
        Ok(Some(out.map(|m| unsafe { m.assume_init() })))
    }
}
