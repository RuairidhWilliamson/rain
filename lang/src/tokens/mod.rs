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
    UnclosedDoubleQuote(LocalSpan),
    IllegalChar(LocalSpan),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenLocalSpan {
    pub token: Token,
    pub span: LocalSpan,
}
