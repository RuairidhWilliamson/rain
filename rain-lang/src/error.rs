use crate::{source::Source, span::Span};

#[derive(Debug, Clone)]
pub struct RainError {
    kind: RainErrorKind,
    span: Span,
}

impl From<crate::tokens::TokenError> for RainError {
    fn from(err: crate::tokens::TokenError) -> Self {
        let span = Span::new_single_byte(err.place);
        Self::new(err, span)
    }
}

#[derive(Debug, Clone)]
pub enum RainErrorKind {
    TokenError(crate::tokens::TokenError),
    ParseError(crate::ast::ParseError),
    ExecError(crate::exec::ExecError),
}

impl From<crate::tokens::TokenError> for RainErrorKind {
    fn from(err: crate::tokens::TokenError) -> Self {
        Self::TokenError(err)
    }
}

impl From<crate::ast::ParseError> for RainErrorKind {
    fn from(err: crate::ast::ParseError) -> Self {
        Self::ParseError(err)
    }
}

impl From<crate::exec::ExecError> for RainErrorKind {
    fn from(err: crate::exec::ExecError) -> Self {
        Self::ExecError(err)
    }
}

impl RainError {
    pub fn new<E: Into<RainErrorKind>>(kind: E, span: Span) -> Self {
        Self {
            kind: kind.into(),
            span,
        }
    }

    pub fn resolve(self, source: &Source) -> ResolvedError {
        ResolvedError { source, err: self }
    }
}

impl std::fmt::Display for RainErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TokenError(err) => err.fmt(f),
            Self::ParseError(err) => err.fmt(f),
            Self::ExecError(err) => err.fmt(f),
        }
    }
}

#[derive(Debug)]
pub struct ResolvedError<'a> {
    source: &'a Source,
    err: RainError,
}

impl std::fmt::Display for ResolvedError<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let span = self.err.span;
        let path = &self.source.path;
        // Line is zero based so we change it to be one based
        let lineno = span.start.line + 1;
        let extract = span.extract(&self.source.source);
        let err = &self.err.kind;
        let under_arrows = extract.under_arrows();
        let line = extract.line;
        f.write_fmt(format_args!(
            "Found error {err}\n{path}:{lineno}\n\t{line}\n\t{under_arrows}\n"
        ))?;
        Ok(())
    }
}
