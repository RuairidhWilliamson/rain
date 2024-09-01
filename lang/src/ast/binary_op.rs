use crate::{
    span::LocalSpan,
    tokens::{Token, TokenLocalSpan},
};

use super::{display::AstDisplay, expr::Expr};

#[derive(Debug)]
pub struct BinaryOp {
    pub left: Box<Expr>,
    pub op: BinaryOperator,
    pub right: Box<Expr>,
}

impl super::display::AstDisplay for BinaryOp {
    fn fmt(&self, f: &mut super::display::AstFormatter<'_>) -> std::fmt::Result {
        f.node("BinaryOp")
            .child(self.left.as_ref())
            .child(&self.op)
            .child(self.right.as_ref())
            .finish()
    }
}

#[derive(PartialEq, Eq)]
pub enum Associativity {
    Left,
    Right,
}

#[derive(Debug)]
pub struct BinaryOperator {
    pub kind: BinaryOperatorKind,
    pub span: LocalSpan,
}

impl AstDisplay for BinaryOperator {
    fn fmt(&self, f: &mut super::display::AstFormatter<'_>) -> std::fmt::Result {
        f.node_single_child(&format!("{:?}", self.kind), &self.span)
    }
}

impl BinaryOperator {
    pub fn new_from_token(t: TokenLocalSpan) -> Option<Self> {
        Some(Self {
            kind: BinaryOperatorKind::new_from_token(t.token)?,
            span: t.span,
        })
    }
}

#[derive(Debug)]
pub enum BinaryOperatorKind {
    Addition,
    Subtraction,
    Multiplication,
    Division,
    Dot,
    LogicalAnd,
    LogicalOr,
    Equals,
    NotEquals,
}

impl BinaryOperatorKind {
    fn new_from_token(t: Token) -> Option<Self> {
        match t {
            Token::Star => Some(Self::Multiplication),
            Token::Slash => Some(Self::Division),
            Token::Plus => Some(Self::Addition),
            Token::Subtract => Some(Self::Subtraction),
            Token::Dot => Some(Self::Dot),
            Token::Equals => Some(Self::Equals),
            Token::NotEquals => Some(Self::NotEquals),
            Token::LogicalAnd => Some(Self::LogicalAnd),
            Token::LogicalOr => Some(Self::LogicalOr),
            _ => None,
        }
    }
}

pub type Precedence = usize;

pub fn get_token_precedence_associativity(token: Token) -> Option<(Precedence, Associativity)> {
    let precedence = match token {
        Token::Dot => Some(70),
        Token::LParen => Some(60),
        Token::Star | Token::Slash => Some(50),
        Token::Plus | Token::Subtract => Some(40),
        Token::Equals | Token::NotEquals => Some(30),
        Token::LogicalAnd => Some(20),
        Token::LogicalOr => Some(10),
        _ => None,
    }?;
    let associativity = Associativity::Left;
    Some((precedence, associativity))
}
