use std::{error::Error, fmt::Display};

#[derive(Debug)]
pub enum PathError {
    Dots,
    Backslash,
    NoParentDirectory,
    NotUnicode,
    IOError(std::io::Error),
}

impl Error for PathError {}

impl Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dots => f.write_str("path cannot contain a segment that only has 3 or more dots"),
            Self::Backslash => f.write_str("path cannot contain backslash"),
            Self::NoParentDirectory => f.write_str("no parent directory"),
            Self::NotUnicode => f.write_str("path is not unicode"),
            Self::IOError(err) => f.write_fmt(format_args!("io error: {err}")),
        }
    }
}

impl From<std::io::Error> for PathError {
    fn from(err: std::io::Error) -> Self {
        Self::IOError(err)
    }
}
