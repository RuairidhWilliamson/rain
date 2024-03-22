use crate::tokens::TokenKind;

mod helpers;

pub mod block;
pub mod declare;
pub mod expr;
pub mod fn_call;
pub mod fn_def;
pub mod ident;
pub mod if_condition;
pub mod item;
pub mod match_expr;
pub mod return_stmt;
pub mod script;
pub mod statement_list;
pub mod stmt;

#[derive(Debug)]
pub enum ParseError {
    EmptyExpression,
    UnexpectedTokens,
    Expected(TokenKind),
    ExpectedAny(Vec<TokenKind>),
    ExpectedStmt,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

trait Ast {
    fn reset_spans(&mut self);
}
