use crate::local_span::LocalSpan;

pub mod peek;
pub mod stream;
#[cfg(test)]
mod test;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    Ident,
    Number,
    SingleQuoteLiteral(Option<StringLiteralPrefix>),
    DoubleQuoteLiteral(Option<StringLiteralPrefix>),
    Comment,
    NewLine,

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
    Hash,

    // Pairs
    LParen,
    RParen,
    LBrace,
    RBrace,
    LAngle,
    RAngle,
    LSqBracket,
    RSqBracket,

    // Compound symbols
    Equals,
    NotEquals,
    LogicalAnd,
    LogicalOr,

    // Keywords that maybe used in the future
    Reserved,
    // Keywords
    Fn,
    Let,
    Pub,
    If,
    Else,
    True,
    False,
    Internal,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenError {
    UnclosedSingleQuote,
    UnclosedDoubleQuote,
    IllegalChar,
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnclosedSingleQuote => f.write_str("unclosed single quotes"),
            Self::UnclosedDoubleQuote => f.write_str("unclosed double quotes"),
            Self::IllegalChar => f.write_fmt(format_args!("illegal char")),
        }
    }
}

impl std::error::Error for TokenError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenLocalSpan {
    pub token: Token,
    pub span: LocalSpan,
}

impl From<TokenLocalSpan> for LocalSpan {
    fn from(tls: TokenLocalSpan) -> Self {
        tls.span
    }
}

impl From<&TokenLocalSpan> for LocalSpan {
    fn from(tls: &TokenLocalSpan) -> Self {
        tls.span
    }
}
