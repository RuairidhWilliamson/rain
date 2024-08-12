use crate::span::LocalSpan;

use super::{Token, TokenError, TokenLocalSpan};

pub struct TokenStream<'a> {
    source: &'a str,
    index: usize,
}

impl<'a> TokenStream<'a> {
    pub fn new(source: &'a str) -> Self {
        Self { source, index: 0 }
    }
}

impl TokenStream<'_> {
    pub fn parse_next(&mut self) -> Result<Option<TokenLocalSpan>, TokenError> {
        loop {
            let Some(c) = self.source.as_bytes().get(self.index) else {
                return Ok(None);
            };
            let tls = match c {
                b'.' => self.inc(Token::Dot),
                b'*' => self.inc(Token::Star),
                b'+' => self.inc(Token::Plus),
                b'-' => self.inc(Token::Subtract),
                b'=' => self.inc(Token::Equals),
                b',' => self.inc(Token::Comma),
                b':' => self.inc(Token::Colon),
                b';' => self.inc(Token::Semicolon),
                b'/' => self.inc(Token::Slash),
                b'\\' => self.inc(Token::Backslash),
                b'~' => self.inc(Token::Tilde),
                b'!' => self.inc(Token::Excalmation),
                b'(' => self.inc(Token::LParen),
                b')' => self.inc(Token::RParen),
                b'{' => self.inc(Token::LBrace),
                b'}' => self.inc(Token::RBrace),
                b'<' => self.inc(Token::LAngle),
                b'>' => self.inc(Token::RAngle),
                b'\n' => self.inc(Token::NewLine),
                b' ' => {
                    self.index += 1;
                    continue;
                }
                b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.ident(),
                b'0'..=b'9' => self.number(),
                b'\"' => self.double_quote_literal()?,
                c if c.is_ascii() => {
                    return Err(TokenError::IllegalChar(LocalSpan::byte(self.index)))
                }
                _ => self.ident(),
            };
            return Ok(Some(tls));
        }
    }

    fn inc(&mut self, token: Token) -> TokenLocalSpan {
        let tls = TokenLocalSpan {
            token,
            span: LocalSpan::byte(self.index),
        };
        self.index += 1;
        tls
    }

    fn ident(&mut self) -> TokenLocalSpan {
        let start = self.index;
        self.index += 1;
        while let Some(c) = self.source.as_bytes().get(self.index) {
            if !self.source.is_char_boundary(self.index) {
                self.index += 1;
                continue;
            }
            match c {
                b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'0'..=b'9' => {}
                c if c.is_ascii() => {
                    break;
                }
                _ => {}
            }
            self.index += 1;
        }
        let s = &self.source[start..self.index];
        let token = match s {
            "fn" => Token::Fn,
            "let" => Token::Let,
            _ => Token::Ident,
        };
        TokenLocalSpan {
            token,
            span: LocalSpan::new(start, self.index),
        }
    }

    fn number(&mut self) -> TokenLocalSpan {
        let start = self.index;
        self.index += 1;
        while let Some(c) = self.source.as_bytes().get(self.index) {
            match c {
                b'0'..=b'9' => {}
                _ => break,
            }
            self.index += 1;
        }
        TokenLocalSpan {
            token: Token::Number,
            span: LocalSpan::new(start, self.index),
        }
    }

    fn double_quote_literal(&mut self) -> Result<TokenLocalSpan, TokenError> {
        let start = self.index;
        self.index += 1;
        loop {
            let Some(c) = self.source.as_bytes().get(self.index) else {
                return Err(TokenError::UnclosedDoubleQuote(LocalSpan::new(
                    start, self.index,
                )));
            };
            if !self.source.is_char_boundary(self.index) {
                self.index += 1;
                continue;
            }
            match c {
                b'\"' => {
                    self.index += 1;
                    break;
                }
                b'\n' => {
                    return Err(TokenError::UnclosedDoubleQuote(LocalSpan::new(
                        start, self.index,
                    )))
                }
                _ => {}
            }
            self.index += 1;
        }
        Ok(TokenLocalSpan {
            token: Token::DoubleQuoteLiteral,
            span: LocalSpan::new(start, self.index),
        })
    }
}

impl Iterator for TokenStream<'_> {
    type Item = Result<TokenLocalSpan, TokenError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.parse_next().transpose()
    }
}
