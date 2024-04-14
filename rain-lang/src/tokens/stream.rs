use crate::span::{Place, Span};

use super::{NextTokenSpan, Token, TokenError, TokenSpan};

#[derive(Debug)]
pub struct TokenStream<'a> {
    source: &'a str,
    raw_source: &'a [u8],
    index: usize,
    line: usize,
    column: usize,
}

impl<'a> Iterator for TokenStream<'a> {
    type Item = Result<TokenSpan<'a>, TokenError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.parse_next() {
            Ok(NextTokenSpan::Next(token_span)) => Some(Ok(token_span)),
            Ok(NextTokenSpan::End(_)) => None,
            Err(err) => Some(Err(err)),
        }
    }
}

impl<'a> TokenStream<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            raw_source: source.as_bytes(),
            index: 0,
            line: 0,
            column: 0,
        }
    }

    pub fn parse_next(&mut self) -> Result<NextTokenSpan<'a>, TokenError> {
        loop {
            let Some(c) = self.raw_source.get(self.index) else {
                return Ok(NextTokenSpan::End(self.span()));
            };
            return match c {
                b'\n' => Ok(self.newline()),
                b'#' => Ok(self.single_line_comment()),
                b'/' => Ok(self.increment(Token::Slash)),
                b'~' => Ok(self.increment(Token::Tilde)),
                b'.' => Ok(self.increment(Token::Dot)),
                b'=' => Ok(self.increment(Token::Equals)),
                b',' => Ok(self.increment(Token::Comma)),
                b':' => Ok(self.increment(Token::Colon)),
                b'\\' => Ok(self.increment(Token::Backslash)),
                b'(' => Ok(self.increment(Token::LParen)),
                b')' => Ok(self.increment(Token::RParen)),
                b'{' => Ok(self.increment(Token::LBrace)),
                b'}' => Ok(self.increment(Token::RBrace)),
                b'[' => Ok(self.increment(Token::LBracket)),
                b']' => Ok(self.increment(Token::RBracket)),
                b'"' => self.double_quotes(),
                b'a'..=b'z' | b'A'..=b'Z' | b'_' => Ok(self.ident()),
                b' ' => {
                    self.index += 1;
                    self.column += 1;
                    continue;
                }
                b'\t' => {
                    self.index += 1;
                    self.column += 1;
                    continue;
                }
                c if !c.is_ascii() => Ok(self.ident()),
                c => Err(TokenError {
                    kind: super::TokenErrorKind::UnknownCharacter(
                        char::from_u32(*c as u32).unwrap(),
                    ),
                    place: Place {
                        index: self.index,
                        line: self.line,
                        column: self.column,
                    },
                }),
            };
        }
    }

    pub fn parse_collect(&mut self) -> Result<Vec<TokenSpan<'a>>, TokenError> {
        self.collect()
    }

    pub fn span(&self) -> Span {
        Span {
            start: Place {
                index: self.index,
                line: self.line,
                column: self.column,
            },
            end: Place {
                index: self.index + 1,
                line: self.line,
                column: self.column + 1,
            },
        }
    }

    fn span_char(&self, token: Token<'a>) -> TokenSpan<'a> {
        TokenSpan {
            token,
            span: self.span(),
        }
    }

    fn increment(&mut self, token: Token<'a>) -> NextTokenSpan<'a> {
        let token_span = self.span_char(token);
        self.index += 1;
        self.column += 1;
        NextTokenSpan::Next(token_span)
    }

    fn newline(&mut self) -> NextTokenSpan<'a> {
        assert_eq!(self.raw_source[self.index], b'\n');
        let token_span = self.span_char(Token::NewLine);
        self.index += 1;
        self.line += 1;
        self.column = 0;
        NextTokenSpan::Next(token_span)
    }

    fn single_line_comment(&mut self) -> NextTokenSpan<'a> {
        for i in self.index..self.raw_source.len() {
            if self.raw_source[i] == b'\n' {
                self.index = i;
                return self.newline();
            }
        }
        NextTokenSpan::End(self.span())
    }

    fn ident(&mut self) -> NextTokenSpan<'a> {
        let start = Place {
            index: self.index,
            line: self.line,
            column: self.column,
        };
        for i in start.index + 1..self.raw_source.len() {
            if !self.source.is_char_boundary(i) {
                continue;
            }
            let c = self.raw_source[i];
            match c {
                b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'0'..=b'9' => {}
                _ => {
                    self.index = i;
                    self.column += self.index - start.index;
                    let span = Span::new(
                        start,
                        Place {
                            index: i,
                            line: start.line,
                            column: self.column,
                        },
                    );
                    return self.keyword_or_ident(&self.source[start.index..i], span);
                }
            }
        }
        self.index = self.source.len();
        self.column += self.index - start.index;
        let span = Span::new(
            start,
            Place {
                index: self.source.len() - 1,
                line: start.line,
                column: self.column - 1,
            },
        );
        self.keyword_or_ident(&self.source[start.index..], span)
    }

    fn keyword_or_ident(&mut self, name: &'a str, span: Span) -> NextTokenSpan<'a> {
        let token = match name {
            "void" => Token::Void,
            "lazy" => Token::Lazy,
            "let" => Token::Let,
            "if" => Token::If,
            "else" => Token::Else,
            "fn" => Token::Fn,
            "return" => Token::Return,
            "match" => Token::Match,
            "true" => Token::TrueLiteral,
            "false" => Token::FalseLiteral,
            _ => Token::Ident(name),
        };
        NextTokenSpan::Next(TokenSpan { token, span })
    }

    fn double_quotes(&mut self) -> Result<NextTokenSpan<'a>, TokenError> {
        let start = Place {
            index: self.index,
            line: self.line,
            column: self.column,
        };
        let mut contents: Vec<u8> = Vec::default();
        let mut escape = false;
        for i in start.index + 1..self.source.len() {
            let c = self.raw_source[i];
            match c {
                b'\n' if escape => {
                    self.line += 1;
                    escape = false;
                }
                _ if escape => {
                    self.column += 1;
                    escape = false;
                    contents.push(c);
                }
                b'"' => {
                    self.index = i + 1;
                    self.column += 2;
                    let end = Place {
                        index: self.index,
                        line: self.line,
                        column: self.column,
                    };
                    return Ok(NextTokenSpan::Next(TokenSpan {
                        token: Token::DoubleQuoteLiteral(String::from_utf8(contents).unwrap()),
                        span: Span { start, end },
                    }));
                }
                b'\n' => {
                    self.line += 1;
                    contents.push(c);
                }
                b'\\' => {
                    self.column += 1;
                    escape = true;
                }
                _ => {
                    self.column += 1;
                    contents.push(c);
                }
            }
        }
        Err(TokenError {
            kind: super::TokenErrorKind::UnclosedDoubleQuote,
            place: start,
        })
    }
}
