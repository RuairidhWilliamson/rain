#[derive(Debug)]
pub enum RunnerError {
    GenericTypeError,
    UnknownIdent,
}

impl std::fmt::Display for RunnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunnerError::GenericTypeError => f.write_str("generic type error"),
            RunnerError::UnknownIdent => f.write_str("unknown identifier"),
        }
    }
}

impl std::error::Error for RunnerError {}
