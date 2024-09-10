#[derive(Debug)]
pub enum RunnerError {
    GenericTypeError,
    UnknownIdent,
    MaxCallDepth,
}

impl std::fmt::Display for RunnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GenericTypeError => f.write_str("generic type error"),
            Self::UnknownIdent => f.write_str("unknown identifier"),
            Self::MaxCallDepth => f.write_str("max_call_depth"),
        }
    }
}

impl std::error::Error for RunnerError {}
