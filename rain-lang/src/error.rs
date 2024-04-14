use crate::{source::Source, span::Span};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RainError {
    pub kind: RainErrorKind,
    pub span: Span,
}

impl From<crate::tokens::TokenError> for RainError {
    fn from(err: crate::tokens::TokenError) -> Self {
        let span = Span::new_single_byte(err.place);
        Self::new(err, span)
    }
}

impl RainError {
    pub fn new<E: Into<RainErrorKind>>(kind: E, span: Span) -> Self {
        Self {
            kind: kind.into(),
            span,
        }
    }

    pub fn resolve(self, source: Source) -> ResolvedError {
        ResolvedError { source, err: self }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

impl std::fmt::Display for RainErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TokenError(err) => err.fmt(f),
            Self::ParseError(err) => err.fmt(f),
            Self::ExecError(err) => err.fmt(f),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedError {
    pub source: Source,
    pub err: RainError,
}

impl std::fmt::Display for ResolvedError {
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
