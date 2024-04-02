use crate::{
    error::{RainError, ResolvedError},
    span::Span,
};

pub mod corelib;
pub mod execution;
pub mod executor;
pub mod external;
pub mod types;

#[derive(Debug, Clone)]
pub enum ExecError {
    UnknownVariable(String),
    UnknownItem(String),
    UnexpectedType {
        expected: &'static [types::RainType],
        actual: types::RainType,
    },
    IncorrectArgCount {
        expected: range_enum::AnyRange<usize>,
        actual: usize,
    },
    Roadmap(&'static str),
}

impl std::fmt::Display for ExecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

#[derive(Debug, Default, Clone)]
pub struct ExecuteOptions {
    pub sealed: bool,
}

/// Control flow rain value
///
/// Returned during execution to control the flow of execution
#[derive(Debug, Clone)]
pub enum ExecCF {
    Return(types::RainValue),
    RuntimeError(RuntimeError),
    RainError(RainError),
    ResolvedRainError(ResolvedError),
}

impl From<RuntimeError> for ExecCF {
    fn from(err: RuntimeError) -> Self {
        Self::RuntimeError(err)
    }
}

impl From<RainError> for ExecCF {
    fn from(err: RainError) -> Self {
        Self::RainError(err)
    }
}

impl From<ResolvedError> for ExecCF {
    fn from(err: ResolvedError) -> Self {
        Self::ResolvedRainError(err)
    }
}

impl ExecCF {
    pub fn map_resolve(self, f: impl FnOnce(RainError) -> Self) -> Self {
        match self {
            ExecCF::RainError(err) => f(err),
            other => other,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeError {
    pub msg: String,
    pub span: Span,
}

impl RuntimeError {
    pub fn new(msg: String, span: Span) -> Self {
        Self { msg, span }
    }
}
