use super::{stream::TokenStream, NextTokenSpan, TokenError};

pub struct PeekTokenStream<'a> {
    stream: TokenStream<'a>,
    peeked: Option<NextTokenSpan<'a>>,
}

impl<'a> From<TokenStream<'a>> for PeekTokenStream<'a> {
    fn from(stream: TokenStream<'a>) -> Self {
        Self {
            stream,
            peeked: None,
        }
    }
}

impl<'a> PeekTokenStream<'a> {
    pub fn new(source: &'a str) -> Self {
        Self::from(TokenStream::new(source))
    }

    pub fn parse_next(&mut self) -> Result<NextTokenSpan<'a>, TokenError> {
        if let Some(peeked) = self.peeked.take() {
            return Ok(peeked);
        }
        self.stream.parse_next()
    }

    pub fn peek(&mut self) -> Result<&NextTokenSpan<'a>, TokenError> {
        if self.peeked.is_none() {
            self.peeked = Some(self.stream.parse_next()?);
        }
        Ok(self.peeked.as_ref().unwrap())
    }
}
