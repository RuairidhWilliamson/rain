use crate::span::LocalSpan;

pub mod peek;
pub mod stream;
#[cfg(test)]
mod test;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    Ident,
    Number,
    DoubleQuoteLiteral(Option<StringLiteralPrefix>),
    Comment,

    // Keywords
    Fn,
    Let,
    If,
    Else,
    True,
    False,
    Internal,

    // Symbols
    Dot,
    Star,
    Plus,
    Subtract,
    Assign,
    Comma,
    Colon,
    Semicolon,
    Slash,
    Backslash,
    Tilde,
    Excalmation,
    Ampersand,
    Pipe,
    Question,
    At,
    Percent,
    Dollar,
    Caret,

    // Pairs
    LParen,
    RParen,
    LBrace,
    RBrace,
    LAngle,
    RAngle,

    // Compound symbols
    Equals,
    NotEquals,
    LogicalAnd,
    LogicalOr,

    NewLine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringLiteralPrefix {
    Format,
}

impl StringLiteralPrefix {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            b'f' => Some(Self::Format),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum TokenError {
    UnclosedDoubleQuote,
    IllegalChar(char),
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnclosedDoubleQuote => f.write_str("unclosed double quotes"),
            Self::IllegalChar(c) => f.write_fmt(format_args!("illegal char {c:?}")),
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
