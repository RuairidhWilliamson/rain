use crate::span::LocalSpan;

pub mod peek;
pub mod stream;
#[cfg(test)]
mod test;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    Ident,
    Number,
    DoubleQuoteLiteral,

    Fn,
    Let,

    Dot,
    Star,
    Plus,
    Subtract,
    Equals,
    Comma,
    Colon,
    Semicolon,
    Slash,
    Backslash,
    Tilde,
    Excalmation,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LAngle,
    RAngle,

    NewLine,
}

#[derive(Debug)]
pub enum TokenError {
    UnclosedDoubleQuote,
    IllegalChar,
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnclosedDoubleQuote => f.write_str("unclosed double quotes"),
            Self::IllegalChar => f.write_str("illegal char"),
        }
    }
}

impl std::error::Error for TokenError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenLocalSpan {
    pub token: Token,
    pub span: LocalSpan,
}

impl TokenLocalSpan {
    pub fn span(self) -> LocalSpan {
        self.span
    }
}
