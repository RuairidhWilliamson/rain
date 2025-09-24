use crate::{
    error::{ResolvedError, ResolvedSpan},
    ir::{ModuleId, Rir},
    local_span::LocalSpan,
    runner::{StacktraceEntry, error::ErrorTrace},
};

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

impl<E: std::error::Error> ErrorSpan<E> {
    pub fn with_trace(self, stacktrace: Vec<StacktraceEntry>) -> ErrorTrace<E> {
        ErrorTrace {
            err_span: self,
            stacktrace,
        }
    }

    pub fn resolve_ir<'a>(&'a self, ir: &'a Rir) -> ResolvedError<'a> {
        let module = ir.get_module(self.span.module);
        let file = module.file().ok();
        let src = &module.src;
        ResolvedError {
            err: &self.err,
            trace: vec![ResolvedSpan {
                file,
                src,
                call_span: self.span.span,
                declaration_span: None,
            }],
        }
    }

    pub fn convert<T>(self) -> ErrorSpan<T>
    where
        T: From<E> + std::error::Error,
    {
        self.span.with_error(T::from(self.err))
    }
}
