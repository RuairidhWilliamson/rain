use crate::span::{Place, Span};

#[derive(Debug)]
pub struct TokenSpan<'a> {
    pub token: Token<'a>,
    pub span: Span,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Token<'a> {
    Ident(&'a str),
    DoubleQuoteLiteral(&'a str),
    BackTickLiteral(&'a str),
    TrueLiteral,
    FalseLiteral,
    Let,
    If,
    Else,
    Fn,
    Dot,
    Assign,
    Comma,
    Colon,
    Slash,
    Tilde,
    LParen,
    RParen,
    LBrace,
    RBrace,
    NewLine,
}

impl TokenSpan<'_> {
    pub fn span(tokens: &[Self]) -> Option<Span> {
        Some(Span::combine(tokens.first()?.span, tokens.last()?.span))
    }
}

#[derive(Debug)]
pub struct TokenError {
    pub char: Option<char>,
    pub place: Place,
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.char, f)
    }
}

impl TokenError {
    pub fn span(&self) -> Span {
        Span::new_single_byte(self.place)
    }
}

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
        self.parse_next().transpose()
    }
}

impl<'a> TokenStream<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            raw_source: dbg!(source.as_bytes()),
            index: 0,
            line: 0,
            column: 0,
        }
    }

    pub fn parse_next(&mut self) -> Result<Option<TokenSpan<'a>>, TokenError> {
        loop {
            let Some(c) = self.raw_source.get(self.index) else {
                return Ok(None);
            };
            dbg!(self.index, c);
            return match c {
                b'\n' => Ok(self.newline()),
                b'#' => Ok(self.single_line_comment()),
                b'/' => Ok(self.increment(Token::Slash)),
                b'~' => Ok(self.increment(Token::Tilde)),
                b'.' => Ok(self.increment(Token::Dot)),
                b'=' => Ok(self.increment(Token::Assign)),
                b',' => Ok(self.increment(Token::Comma)),
                b':' => Ok(self.increment(Token::Colon)),
                b'(' => Ok(self.increment(Token::LParen)),
                b')' => Ok(self.increment(Token::RParen)),
                b'{' => Ok(self.increment(Token::LBrace)),
                b'}' => Ok(self.increment(Token::RBrace)),
                b'"' => Ok(self.double_quotes()),
                b'`' => Ok(self.back_ticks()),
                b'a'..=b'z' | b'A'..=b'Z' | b'_' => Ok(self.ident()),
                b' ' => {
                    self.index += 1;
                    self.column += 1;
                    continue;
                }
                c if !c.is_ascii() => Ok(self.ident()),
                c => Err(TokenError {
                    char: char::from_u32(*c as u32),
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

    fn span_char(&self, token: Token<'a>) -> TokenSpan<'a> {
        TokenSpan {
            token,
            span: Span {
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
            },
        }
    }

    fn increment(&mut self, token: Token<'a>) -> Option<TokenSpan<'a>> {
        let span = self.span_char(token);
        self.index += 1;
        self.column += 1;
        Some(span)
    }

    fn newline(&mut self) -> Option<TokenSpan<'a>> {
        assert_eq!(self.raw_source[self.index], b'\n');
        let span = self.span_char(Token::NewLine);
        self.index += 1;
        self.line += 1;
        self.column = 0;
        Some(span)
    }

    fn single_line_comment(&mut self) -> Option<TokenSpan<'a>> {
        for i in self.index..self.raw_source.len() {
            if self.raw_source[i] == b'\n' {
                self.index = i;
                return self.newline();
            }
        }
        None
    }

    fn ident(&mut self) -> Option<TokenSpan<'a>> {
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

    fn keyword_or_ident(&mut self, name: &'a str, span: Span) -> Option<TokenSpan<'a>> {
        let token = match name {
            "let" => Token::Let,
            "if" => Token::If,
            "else" => Token::Else,
            "fn" => Token::Fn,
            "true" => Token::TrueLiteral,
            "false" => Token::FalseLiteral,
            _ => Token::Ident(name),
        };
        Some(TokenSpan { token, span })
    }

    fn double_quotes(&mut self) -> Option<TokenSpan<'a>> {
        let start = Place {
            index: self.index,
            line: self.line,
            column: self.column,
        };
        for i in start.index + 1..self.source.len() {
            let c = self.raw_source[i];
            match c {
                b'"' => {
                    self.index = i + 1;
                    self.column += 2;
                    let end = Place {
                        index: self.index,
                        line: self.line,
                        column: self.column,
                    };
                    return Some(TokenSpan {
                        token: Token::DoubleQuoteLiteral(&self.source[start.index + 1..i]),
                        span: Span { start, end },
                    });
                }
                b'\n' => {
                    self.line += 1;
                }
                _ => {
                    self.column += 1;
                }
            }
        }
        panic!("error missing closing double quote")
    }

    fn back_ticks(&mut self) -> Option<TokenSpan<'a>> {
        let start = Place {
            index: self.index,
            line: self.line,
            column: self.column,
        };
        for i in start.index + 1..self.source.len() {
            let c = self.raw_source[i];
            match c {
                b'`' => {
                    self.index = i + 1;
                    self.column += 2;
                    let end = Place {
                        index: self.index,
                        line: self.line,
                        column: self.column,
                    };
                    return Some(TokenSpan {
                        token: Token::BackTickLiteral(&self.source[start.index + 1..i]),
                        span: Span { start, end },
                    });
                }
                b'\n' => {
                    self.line += 1;
                }
                _ => {
                    self.column += 1;
                }
            }
        }
        panic!("error missing closing back tick")
    }
}

#[cfg(test)]
mod tests {
    use super::{Token, TokenStream};

    #[test]
    fn tokens_assignment() {
        let source = "let a = \"abc\"";
        let tokens: Vec<Token> = TokenStream::new(source)
            .map(|ts| ts.unwrap().token)
            .collect();

        assert_eq!(
            tokens,
            vec![
                Token::Let,
                Token::Ident("a"),
                Token::Assign,
                Token::DoubleQuoteLiteral("abc")
            ],
        )
    }

    #[test]
    fn token_fn_declaration() {
        let source = "fn foo() {
            std.print(\"Hello :)\")
        }";
        let tokens: Vec<Token> = TokenStream::new(source)
            .map(|ts| ts.unwrap().token)
            .filter(|t| !matches!(t, Token::NewLine))
            .collect();
        assert_eq!(
            tokens,
            vec![
                Token::Fn,
                Token::Ident("foo"),
                Token::LParen,
                Token::RParen,
                Token::LBrace,
                Token::Ident("std"),
                Token::Dot,
                Token::Ident("print"),
                Token::LParen,
                Token::DoubleQuoteLiteral("Hello :)"),
                Token::RParen,
                Token::RBrace,
            ]
        )
    }

    #[test]
    fn tokens_hello_world() {
        let source = "std.print(\"hello world\")";
        let tokens: Vec<Token> = TokenStream::new(source)
            .map(|ts| ts.unwrap().token)
            .collect();

        assert_eq!(
            tokens,
            vec![
                Token::Ident("std"),
                Token::Dot,
                Token::Ident("print"),
                Token::LParen,
                Token::DoubleQuoteLiteral("hello world"),
                Token::RParen
            ]
        );
    }

    #[test]
    fn tokens_multiline() {
        let source = "std.print()\nstd.print()";
        let tokens: Vec<Token> = TokenStream::new(source)
            .map(|ts| ts.unwrap().token)
            .collect();

        assert_eq!(
            tokens,
            vec![
                Token::Ident("std"),
                Token::Dot,
                Token::Ident("print"),
                Token::LParen,
                Token::RParen,
                Token::NewLine,
                Token::Ident("std"),
                Token::Dot,
                Token::Ident("print"),
                Token::LParen,
                Token::RParen,
            ]
        );
    }

    #[test]
    fn tokens_comment() {
        let source = "std.print()\n# This should not be tokens\nstd.print()";
        let tokens: Vec<Token> = TokenStream::new(source)
            .map(|ts| ts.unwrap().token)
            .collect();

        assert_eq!(
            tokens,
            vec![
                Token::Ident("std"),
                Token::Dot,
                Token::Ident("print"),
                Token::LParen,
                Token::RParen,
                Token::NewLine,
                Token::NewLine,
                Token::Ident("std"),
                Token::Dot,
                Token::Ident("print"),
                Token::LParen,
                Token::RParen,
            ]
        )
    }

    #[test]
    fn tokens_back_ticks() {
        let source = "let a = `./a.txt`";
        let tokens: Vec<Token> = TokenStream::new(source)
            .map(|ts| ts.unwrap().token)
            .collect();
        assert_eq!(
            tokens,
            vec![
                Token::Let,
                Token::Ident("a"),
                Token::Assign,
                Token::BackTickLiteral("./a.txt")
            ]
        )
    }

    #[test]
    fn tokens_emoji() {
        let source = "let 🦀 = \"🦀\"";
        let tokens: Vec<Token> = TokenStream::new(source)
            .map(|ts| ts.unwrap().token)
            .collect();
        assert_eq!(
            tokens,
            vec![
                Token::Let,
                Token::Ident("🦀"),
                Token::Assign,
                Token::DoubleQuoteLiteral("🦀"),
            ]
        )
    }

    #[test]
    fn tokens_emoji2() {
        let source = "let 🌧 = \"rain\"";
        let tokens: Vec<Token> = TokenStream::new(source)
            .map(|ts| ts.unwrap().token)
            .collect();
        assert_eq!(
            tokens,
            vec![
                Token::Let,
                Token::Ident("🌧"),
                Token::Assign,
                Token::DoubleQuoteLiteral("rain"),
            ]
        )
    }

    #[test]
    fn tokens_column() {
        let source = "std.print(\"hello world\")";
        let token_span = TokenStream::new(source).last().unwrap().unwrap();
        assert_eq!(token_span.span.start.column, source.len() - 1);
    }
}
