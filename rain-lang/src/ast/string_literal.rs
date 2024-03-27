use crate::span::Span;

use super::Ast;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringLiteral {
    pub value: String,
    pub span: Span,
}

impl StringLiteral {
    pub fn nosp(value: &str) -> Self {
        Self {
            value: String::from(value),
            span: Span::default(),
        }
    }
}

impl Ast for StringLiteral {
    fn span(&self) -> Span {
        self.span
    }

    fn reset_spans(&mut self) {
        self.span.reset()
    }
}
