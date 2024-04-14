use crate::{
    error::{RainError, ResolvedError},
    span::Span,
};

pub mod corelib;
pub mod execution;
pub mod executor;
pub mod external;
pub mod types;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecError {
    UnknownItem(String),
    UnexpectedType {
        expected: &'static [types::RainType],
        actual: types::RainType,
    },
    IncorrectArgCount {
        expected: range_enum::AnyRange<usize>,
        actual: usize,
    },
    ReturnOutsideFunction,
    CallDepthLimit,
    Roadmap(&'static str),
}

impl std::fmt::Display for ExecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

#[derive(Debug, Clone)]
pub struct ExecuteOptions {
    pub sealed: bool,
    pub call_depth_limit: usize,
}

impl Default for ExecuteOptions {
    fn default() -> Self {
        Self {
            sealed: false,
            call_depth_limit: 32,
        }
    }
}

/// Control flow rain value
///
/// Returned during execution to control the flow of execution
#[derive(Debug, Clone)]
pub enum ExecCF {
    Return(types::RainValue, Span),
    RuntimeError(RuntimeError),
    RainError(RainError),
    // Box error to reduce sizeof ExecCF
    ResolvedRainError(Box<ResolvedError>),
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
        Self::ResolvedRainError(Box::new(err))
    }
}

impl ExecCF {
    pub fn map_resolve(self, f: impl FnOnce(RainError) -> Self) -> Self {
        match self {
            Self::RainError(err) => f(err),
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
