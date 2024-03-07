use crate::tokens::TokenKind;

pub mod declare;
pub mod expr;
pub mod fn_call;
pub mod fn_def;
mod helpers;
pub mod ident;
pub mod item;
pub mod script;
pub mod stmt;

#[derive(Debug)]
pub enum ParseError {
    EmptyExpression,
    UnexpectedTokens,
    ExpectedAssignToken,
    ExpectedIdent,
    ExpectedFn,
    ExpectedLParen,
    ExpectedRBrace,
    Expected(TokenKind),
    ExpectedAny(Vec<TokenKind>),
    ExpectedStmt,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}
