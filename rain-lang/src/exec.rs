pub mod corelib;
pub mod executable;
pub mod executor;
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
        expected: usize,
        actual: usize,
    },
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
