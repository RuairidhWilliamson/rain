use std::{borrow::Cow, ops::RangeInclusive, string::FromUtf8Error};

use crate::{
    afs::{entry::FSEntry, error::PathError},
    ast::error::ParseError,
    driver::FSEntryQueryResult,
    error::{ResolvedError, ResolvedSpan},
    ir::Rir,
    runner::cx::StacktraceEntry,
    span::ErrorSpan,
};

use super::value::{RainInteger, RainTypeId};

#[derive(Debug)]
pub struct ErrorTrace<E: std::error::Error> {
    pub err_span: ErrorSpan<E>,
    pub stacktrace: Vec<StacktraceEntry>,
}

impl<E: std::error::Error> ErrorTrace<E> {
    pub fn resolve_ir<'a>(&'a self, ir: &'a Rir) -> ResolvedError<'a> {
        let mut trace = Vec::new();
        for s in &self.stacktrace {
            let module = ir.get_module(s.m);
            let span = module.span(s.n);
            let file = module.file().ok();
            let src = &module.src;
            trace.push(ResolvedSpan {
                file,
                src,
                call_span: span,
            });
        }
        let module = ir.get_module(self.err_span.span.module);
        let file = module.file().ok();
        let src = &module.src;
        trace.push(ResolvedSpan {
            file,
            src,
            call_span: self.err_span.span.span,
        });
        ResolvedError {
            err: &self.err_span.err,
            trace,
        }
    }

    pub fn convert<T>(self) -> ErrorTrace<T>
    where
        T: From<E> + std::error::Error,
    {
        ErrorTrace {
            err_span: self.err_span.convert(),
            stacktrace: self.stacktrace,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Throwing {
    #[error("{0}")]
    Recoverable(super::value::Value),
    #[error("unrecoverable error: {0}")]
    Unrecoverable(#[from] RunnerError),
}

impl From<ParseError> for Throwing {
    fn from(err: ParseError) -> Self {
        Self::Unrecoverable(RunnerError::from(err))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RunnerError {
    #[error("makeshift: {0}")]
    Makeshift(Cow<'static, str>),
    #[error("makeshift io: {0}: {1}")]
    MakeshiftIO(Cow<'static, str>, std::io::Error),
    #[error("wrong number of args, required {required:?} but got {actual}")]
    IncorrectArgs {
        required: RangeInclusive<usize>,
        actual: usize,
    },
    #[error("unknown identifier")]
    UnknownIdent,
    #[error("type mismatch, expected {expected:?} actual {actual:?}")]
    ExpectedType {
        actual: RainTypeId,
        expected: &'static [RainTypeId],
    },
    #[error("invalid integer literal")]
    InvalidIntegerLiteral,
    #[error("reached max call depth possibly due to infinite recursion")]
    MaxCallDepth,
    #[error("path error: {0}")]
    PathError(#[from] PathError),
    #[error("local areas can only be created from local areas")]
    IllegalLocalArea,
    #[error("io error when getting area: {0}")]
    AreaIOError(std::io::Error),
    #[error("io error when importing: {0}")]
    ImportIOError(std::io::Error),
    #[error("parse error when importing: {0}")]
    ImportParseError(#[from] ParseError),
    #[error("zip error: {0}")]
    ExtractError(Box<dyn std::error::Error>),
    #[error("fs query path {0} {1}")]
    FSQuery(FSEntry, FSEntryQueryResult),
    #[error("record does not contain entry: {name}")]
    RecordMissingEntry { name: String },
    #[error("index out of bounds: {0}")]
    IndexOutOfBounds(RainInteger),
    #[error("index key not found: {0}")]
    IndexKeyNotFound(String),
    #[error("cannot call from prelude")]
    PreludeContext,
    #[error("can't escape seal 🦭")]
    CantEscapeSeal,
    #[error("{0}")]
    FromUtf8Error(#[from] FromUtf8Error),
}
