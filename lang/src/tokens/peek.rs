use std::collections::VecDeque;

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

    pub fn parse_next(&mut self) -> Result<Option<TokenLocalSpan>, TokenError> {
        if let Some(peeked) = self.peeked.pop_front() {
            return Ok(Some(peeked));
        }
        self.stream.parse_next()
    }

    pub fn peek(&mut self) -> Result<Option<TokenLocalSpan>, TokenError> {
        if self.peeked.is_empty() {
            let Some(tls) = self.stream.parse_next()? else {
                return Ok(None);
            };
            self.peeked.push_back(tls);
        }
        Ok(self.peeked.front().copied())
    }
}
