use crate::{error::ErrorSpan, span::LocalSpan};

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
    pub fn parse_next(&mut self) -> Result<Option<TokenLocalSpan>, ErrorSpan<TokenError>> {
        loop {
            let bytes = self.source.as_bytes();
            let Some(&c) = bytes.get(self.index) else {
                return Ok(None);
            };
            let c_next = bytes.get(self.index + 1);
            let tls = match (c, c_next) {
                (b'/', Some(b'/')) => self.comment(),
                (b'.', _) => self.inc(Token::Dot),
                (b'*', _) => self.inc(Token::Star),
                (b'+', _) => self.inc(Token::Plus),
                (b'-', _) => self.inc(Token::Subtract),
                (b',', _) => self.inc(Token::Comma),
                (b':', _) => self.inc(Token::Colon),
                (b';', _) => self.inc(Token::Semicolon),
                (b'/', _) => self.inc(Token::Slash),
                (b'\\', _) => self.inc(Token::Backslash),
                (b'~', _) => self.inc(Token::Tilde),
                (b'?', _) => self.inc(Token::Question),
                (b'@', _) => self.inc(Token::At),
                (b'%', _) => self.inc(Token::Percent),
                (b'$', _) => self.inc(Token::Dollar),
                (b'^', _) => self.inc(Token::Caret),
                (b'(', _) => self.inc(Token::LParen),
                (b')', _) => self.inc(Token::RParen),
                (b'{', _) => self.inc(Token::LBrace),
                (b'}', _) => self.inc(Token::RBrace),
                (b'<', _) => self.inc(Token::LAngle),
                (b'>', _) => self.inc(Token::RAngle),
                (b'|', Some(b'|')) => self.inc2(Token::LogicalOr),
                (b'|', _) => self.inc(Token::Pipe),
                (b'&', Some(b'&')) => self.inc2(Token::LogicalAnd),
                (b'&', _) => self.inc(Token::Ampersand),
                (b'=', Some(b'=')) => self.inc2(Token::Equals),
                (b'=', _) => self.inc(Token::Assign),
                (b'!', Some(b'=')) => self.inc2(Token::NotEquals),
                (b'!', _) => self.inc(Token::Excalmation),
                (b'\n', _) => self.inc(Token::NewLine),
                (b' ' | b'\t', _) => {
                    self.index += 1;
                    continue;
                }
                (b'a'..=b'z', Some(b'\"')) => self.double_quote_literal()?,
                (b'\"', _) => self.double_quote_literal()?,
                (b'a'..=b'z' | b'A'..=b'Z' | b'_', _) => self.ident(),
                (b'0'..=b'9', _) => self.number(),
                (c, _) if c.is_ascii() => {
                    return Err(LocalSpan::byte(self.index)
                        .with_error(TokenError::IllegalChar(char::from(c))));
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

    fn inc2(&mut self, token: Token) -> TokenLocalSpan {
        let tls = TokenLocalSpan {
            token,
            span: LocalSpan::new(self.index, self.index + 2),
        };
        self.index += 2;
        tls
    }

    fn comment(&mut self) -> TokenLocalSpan {
        let start = self.index;
        self.index += 2;
        while let Some(&c) = self.source.as_bytes().get(self.index) {
            if b'\n' == c {
                break;
            }
            self.index += 1;
        }
        TokenLocalSpan {
            token: Token::Comment,
            span: LocalSpan::new(start, self.index),
        }
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
            "true" => Token::True,
            "false" => Token::False,
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

    fn double_quote_literal(&mut self) -> Result<TokenLocalSpan, ErrorSpan<TokenError>> {
        let start = self.index;
        if Some(b'"') != self.source.as_bytes().get(self.index).copied() {
            // Skip over the string modifier
            self.index += 1;
        }
        self.index += 1;
        loop {
            let Some(c) = self.source.as_bytes().get(self.index) else {
                return Err(
                    LocalSpan::new(start, self.index).with_error(TokenError::UnclosedDoubleQuote)
                );
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
                    return Err(LocalSpan::new(start, self.index)
                        .with_error(TokenError::UnclosedDoubleQuote))
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
    type Item = Result<TokenLocalSpan, ErrorSpan<TokenError>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.parse_next().transpose()
    }
}
