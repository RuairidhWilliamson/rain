use crate::span::Span;

use super::Ast;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoolLiteral {
    pub value: bool,
    pub span: Span,
}

impl BoolLiteral {
    pub fn nosp(value: bool) -> Self {
        Self {
            value,
            span: Span::default(),
        }
    }
}

impl Ast for BoolLiteral {
    fn span(&self) -> Span {
        self.span
    }

    fn reset_spans(&mut self) {
        self.span.reset()
    }
}
