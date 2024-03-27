use crate::{span::Span, tokens::TokenKind};

mod helpers;

pub mod block;
pub mod bool_literal;
pub mod declare;
pub mod dot;
pub mod expr;
pub mod function_call;
pub mod function_def;
pub mod ident;
pub mod if_condition;
pub mod match_expr;
pub mod return_stmt;
pub mod script;
pub mod statement;
pub mod statement_list;
pub mod string_literal;

#[derive(Debug, Clone)]
pub enum ParseError {
    EmptyExpression,
    UnexpectedTokens,
    Expected(TokenKind),
    ExpectedAny(&'static [TokenKind]),
    ExpectedStmt,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

pub trait Ast: std::fmt::Debug + Clone + Eq {
    fn span(&self) -> Span;

    fn reset_spans(&mut self);
}
