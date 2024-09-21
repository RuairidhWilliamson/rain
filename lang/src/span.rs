use crate::{
    error::ResolvedError,
    ir::{ModuleId, Rir},
    local_span::LocalSpan,
};

#[derive(Debug)]
pub struct Span {
    pub module: ModuleId,
    pub span: Option<LocalSpan>,
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

impl<E: std::error::Error> ErrorSpan<E> {
    pub fn resolve_ir<'a>(&'a self, ir: &'a Rir) -> ResolvedError<'a> {
        let module = ir.get_module(self.span.module);
        let path = module.path.as_deref();
        let src = &module.src;
        ResolvedError {
            err: &self.err,
            path,
            src,
            span: self.span.span,
        }
    }
}
