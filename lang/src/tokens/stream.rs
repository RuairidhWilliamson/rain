use crate::local_span::{ErrorLocalSpan, LocalSpan};

use super::{StringLiteralPrefix, Token, TokenError, TokenLocalSpan};

pub struct TokenStream<'a> {
    source: &'a str,
    index: usize,
    last_parsed_span: Option<LocalSpan>,
}

impl<'a> TokenStream<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            index: 0,
            last_parsed_span: None,
        }
    }
}

impl TokenStream<'_> {
    pub fn parse_next(&mut self) -> Result<Option<TokenLocalSpan>, ErrorLocalSpan<TokenError>> {
        // Turns Token::Reserved into an error
        match self.parse_next_inner() {
            Ok(Some(TokenLocalSpan {
                token: Token::Reserved,
                span,
            })) => Err(span.with_error(TokenError::ReservedKeyword)),
            other => other,
        }
    }

    fn parse_next_inner(&mut self) -> Result<Option<TokenLocalSpan>, ErrorLocalSpan<TokenError>> {
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
                (b'-', Some(b'>')) => self.inc2(Token::ReturnType),
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
                (b'#', _) => self.inc(Token::Hash),
                (b'(', _) => self.inc(Token::LParen),
                (b')', _) => self.inc(Token::RParen),
                (b'{', _) => self.inc(Token::LBrace),
                (b'}', _) => self.inc(Token::RBrace),
                (b'<', Some(b'=')) => self.inc2(Token::LessEq),
                (b'>', Some(b'=')) => self.inc2(Token::GreaterEq),
                (b'<', _) => self.inc(Token::LAngle),
                (b'>', _) => self.inc(Token::RAngle),
                (b'[', _) => self.inc(Token::LSqBracket),
                (b']', _) => self.inc(Token::RSqBracket),
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
                (b'a'..=b'z', Some(b'\'')) | (b'\'', _) => self.single_quote_literal()?,
                (b'a'..=b'z', Some(b'"')) | (b'"', _) => self.double_quote_literal()?,
                (b'a'..=b'z' | b'A'..=b'Z' | b'_', _) => self.ident(),
                (b'0'..=b'9', _) => self.number(),
                (c, _) if c.is_ascii() => {
                    return Err(
                        LocalSpan::byte(self.index).with_error(TokenError::IllegalAsciiChar(c))
                    );
                }
                _ => self.ident(),
            };
            self.last_parsed_span = Some(tls.span);
            return Ok(Some(tls));
        }
    }

    pub fn last_span(&self) -> LocalSpan {
        self.last_parsed_span.unwrap_or_default()
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
            "pub" => Token::Pub,
            "fn" => Token::Fn,
            "let" => Token::Let,
            "if" => Token::If,
            "else" => Token::Else,
            "true" => Token::True,
            "false" => Token::False,
            "internal" => Token::Internal,
            "import" => Token::Import,
            "stdlib" => Token::Stdlib,
            "this_file" => Token::ThisFile,
            "throw" | "try" | "type" | "for" | "in" | "while" | "match" | "record" | "async"
            | "await" | "default" | "struct" | "trait" | "break" | "continue" | "return"
            | "yield" | "enum" | "union" | "safe" | "unsafe" | "macro" | "const" | "var"
            | "interface" | "abstract" | "alias" | "super" => Token::Reserved,
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

    fn single_quote_literal(&mut self) -> Result<TokenLocalSpan, ErrorLocalSpan<TokenError>> {
        let start = self.index;
        let prefix_symbol = self.source.as_bytes().get(self.index).copied();
        let prefix = match prefix_symbol {
            Some(b'\'') => None,
            Some(b @ b'a'..=b'z') => {
                let Some(prefix) = StringLiteralPrefix::from_byte(b) else {
                    return Err(
                        LocalSpan::byte(self.index).with_error(TokenError::BadStringLiteralPrefix)
                    );
                };
                // Skip over the string modifier
                self.index += 1;
                Some(prefix)
            }
            _ => unreachable!("single_quote_literal"),
        };
        self.index += 1;
        let mut escape = false;
        loop {
            let Some(c) = self.source.as_bytes().get(self.index) else {
                return Err(
                    LocalSpan::new(start, self.index).with_error(TokenError::UnclosedSingleQuote)
                );
            };
            if !self.source.is_char_boundary(self.index) {
                self.index += 1;
                continue;
            }
            if escape {
                self.index += 1;
                escape = false;
                continue;
            }
            match c {
                b'\\' if prefix != Some(StringLiteralPrefix::Raw) => {
                    escape = true;
                }
                b'\'' => {
                    self.index += 1;
                    break;
                }
                b'\n' => {
                    return Err(LocalSpan::new(start, self.index)
                        .with_error(TokenError::UnclosedSingleQuote));
                }
                _ => {}
            }
            self.index += 1;
        }
        Ok(TokenLocalSpan {
            token: Token::SingleQuoteLiteral(prefix),
            span: LocalSpan::new(start, self.index),
        })
    }

    fn double_quote_literal(&mut self) -> Result<TokenLocalSpan, ErrorLocalSpan<TokenError>> {
        let start = self.index;
        let prefix_symbol = self.source.as_bytes().get(self.index).copied();
        let prefix = match prefix_symbol {
            Some(b'"') => None,
            Some(b @ b'a'..=b'z') => {
                let Some(prefix) = StringLiteralPrefix::from_byte(b) else {
                    return Err(
                        LocalSpan::byte(self.index).with_error(TokenError::BadStringLiteralPrefix)
                    );
                };
                // Skip over the string modifier
                self.index += 1;
                Some(prefix)
            }
            _ => unreachable!("double_quote_literal"),
        };
        self.index += 1;
        let mut escape = false;
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
            if escape {
                self.index += 1;
                escape = false;
                continue;
            }
            match c {
                b'\\' if prefix != Some(StringLiteralPrefix::Raw) => {
                    escape = true;
                }
                b'"' => {
                    self.index += 1;
                    break;
                }
                b'\n' => {
                    return Err(LocalSpan::new(start, self.index)
                        .with_error(TokenError::UnclosedDoubleQuote));
                }
                _ => {}
            }
            self.index += 1;
        }
        Ok(TokenLocalSpan {
            token: Token::DoubleQuoteLiteral(prefix),
            span: LocalSpan::new(start, self.index),
        })
    }
}

impl Iterator for TokenStream<'_> {
    type Item = Result<TokenLocalSpan, ErrorLocalSpan<TokenError>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.parse_next().transpose()
    }
}
