use crate::span::Span;

use super::Ast;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringLiteral<'a> {
    pub value: &'a str,
    pub span: Span,
}

impl<'a> StringLiteral<'a> {
    pub fn nosp(value: &'a str) -> Self {
        Self {
            value,
            span: Span::default(),
        }
    }
}

impl Ast for StringLiteral<'_> {
    fn span(&self) -> Span {
        self.span
    }

    fn reset_spans(&mut self) {
        self.span.reset()
    }
}
