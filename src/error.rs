use std::path::Path;

use crate::span::Span;

#[derive(Debug)]
pub enum RainError {
    TokenError(crate::tokens::TokenError),
    ParseError(crate::ast::ParseError),
    ExecError(crate::exec::ExecError),
}

impl From<crate::tokens::TokenError> for RainError {
    fn from(err: crate::tokens::TokenError) -> Self {
        Self::TokenError(err)
    }
}

impl From<crate::ast::ParseError> for RainError {
    fn from(err: crate::ast::ParseError) -> Self {
        Self::ParseError(err)
    }
}

impl From<crate::exec::ExecError> for RainError {
    fn from(err: crate::exec::ExecError) -> Self {
        Self::ExecError(err)
    }
}

impl RainError {
    pub fn resolve<'a>(self, source_path: &'a Path, source: &'a str) -> ResolvedError<'a> {
        ResolvedError {
            source,
            source_path,
            err: self,
        }
    }

    pub fn span(&self) -> Span {
        match self {
            RainError::TokenError(err) => err.span(),
            RainError::ParseError(err) => err.span(),
            RainError::ExecError(err) => err.span(),
        }
    }
}

impl std::fmt::Display for RainError {
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
    source: &'a str,
    source_path: &'a Path,
    err: RainError,
}

impl std::fmt::Display for ResolvedError<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let span = self.err.span();
        let path = self.source_path.display();
        // Line is zero based so we change it to be one based
        let line = span.start.line + 1;
        let extract = &self.source[span.start.index..span.end.index];
        let err = &self.err;
        f.write_fmt(format_args!(
            "Found error in {path}:{line}\n\t{extract}\n{err}\n"
        ))?;
        Ok(())
    }
}
