use crate::{area::PathError, ast::error::ParseError};

use super::value::RainTypeId;

#[derive(Debug)]
pub enum RunnerError {
    GenericTypeError,
    UnknownIdent,
    ExpectedType(RainTypeId, &'static [RainTypeId]),
    InvalidIntegerLiteral,
    MaxCallDepth,
    PathError(PathError),
    ImportResolve,
    ImportIOError(std::io::Error),
    ImportParseError(ParseError),
}

impl std::fmt::Display for RunnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GenericTypeError => f.write_str("generic type error"),
            Self::UnknownIdent => f.write_str("unknown identifier"),
            Self::ExpectedType(actual, expected) => f.write_fmt(format_args!(
                "type mismatch, expected {expected:?} actual {actual:?}"
            )),
            Self::InvalidIntegerLiteral => f.write_str("invalid integer literal"),
            Self::MaxCallDepth => {
                f.write_str("reached max call depth probably due to infinite recursion")
            }
            Self::PathError(err) => f.write_fmt(format_args!("path error: {err}")),
            Self::ImportResolve => f.write_fmt(format_args!("could not resolve import")),
            Self::ImportIOError(err) => f.write_fmt(format_args!("io error when importing: {err}")),
            Self::ImportParseError(err) => {
                f.write_fmt(format_args!("parse error when importing: {err}"))
            }
        }
    }
}

impl std::error::Error for RunnerError {}

impl From<ParseError> for RunnerError {
    fn from(err: ParseError) -> Self {
        Self::ImportParseError(err)
    }
}

impl From<PathError> for RunnerError {
    fn from(err: PathError) -> Self {
        Self::PathError(err)
    }
}
