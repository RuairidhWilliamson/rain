use std::collections::VecDeque;

use super::{stream::TokenStream, NextTokenSpan, TokenError};

#[derive(Debug)]
pub struct PeekTokenStream<'a> {
    stream: TokenStream<'a>,
    peeked: VecDeque<NextTokenSpan<'a>>,
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

    pub fn parse_next(&mut self) -> Result<NextTokenSpan<'a>, TokenError> {
        if let Some(peeked) = self.peeked.pop_front() {
            return Ok(peeked);
        }
        self.stream.parse_next()
    }

    pub fn peek<'b>(&'b mut self) -> Result<PeekNextTokenSpan<'a, 'b>, TokenError> {
        if self.peeked.is_empty() {
            self.peeked.push_back(self.stream.parse_next()?);
        }
        Ok(PeekNextTokenSpan { stream: self })
    }

    pub fn peek_many<'b>(&'b mut self, n: usize) -> Result<PeekManyTokenSpan<'a, 'b>, TokenError> {
        while self.peeked.len() < n {
            self.peeked.push_back(self.stream.parse_next()?);
        }
        Ok(PeekManyTokenSpan { stream: self })
    }
}

#[derive(Debug)]
pub struct PeekNextTokenSpan<'a, 'b> {
    stream: &'b mut PeekTokenStream<'a>,
}

impl<'a, 'b> PeekNextTokenSpan<'a, 'b> {
    pub fn value(&self) -> &NextTokenSpan<'a> {
        self.stream.peeked.front().unwrap()
    }

    pub fn consume(self) -> NextTokenSpan<'a> {
        self.stream.parse_next().unwrap()
    }
}

#[derive(Debug)]
pub struct PeekManyTokenSpan<'a, 'b> {
    stream: &'b mut PeekTokenStream<'a>,
}

impl<'a, 'b> PeekManyTokenSpan<'a, 'b> {
    pub fn get(&self, index: usize) -> &NextTokenSpan<'a> {
        self.stream.peeked.get(index).unwrap()
    }
}
