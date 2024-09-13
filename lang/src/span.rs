use crate::{ir::ModuleId, local_span::LocalSpan};

#[derive(Debug)]
pub struct Span {
    pub module: ModuleId,
    pub span: LocalSpan,
}

impl Span {
    pub const fn with_error<E: std::error::Error>(self, err: E) -> ErrorSpan<E> {
        ErrorSpan { err, span: self }
    }
}

#[derive(Debug)]
pub struct ErrorSpan<E: std::error::Error> {
    pub err: E,
    pub span: Span,
}
