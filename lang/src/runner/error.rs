use std::{borrow::Cow, ops::RangeInclusive};

use crate::{
    afs::{entry::FSEntry, error::PathError},
    ast::error::ParseError,
    driver::FSEntryQueryResult,
};

use super::value::{RainInteger, RainTypeId};

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
    #[error("could not resolve import")]
    ImportResolve,
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
}
