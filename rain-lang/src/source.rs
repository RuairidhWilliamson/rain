use std::rc::Rc;

use crate::path::{RainPath, RainPathScope};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub path: SourcePath,
    pub source: Rc<str>,
}

impl Source {
    pub fn new(path: &RainPath) -> Result<Self, std::io::Error> {
        let f = std::fs::File::open(path.resolve())?;
        let metadata = f.metadata()?;
        if metadata.is_dir() {
            let new_path = path.join("main.rain");
            tracing::debug!("{path:?} is a directory using {new_path:?}");
            let source = std::fs::read_to_string(new_path.resolve())?;
            Ok(Self {
                source: source.into(),
                path: SourcePath::FilePath { path: new_path },
            })
        } else {
            let source = std::io::read_to_string(f)?;
            Ok(Self {
                source: source.into(),
                path: SourcePath::FilePath {
                    path: path.to_owned(),
                },
            })
        }
    }

    pub fn new_evaluated(directory: RainPath, source: String) -> Self {
        Self {
            path: SourcePath::Evaluated { directory },
            source: source.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourcePath {
    FilePath { path: RainPath },
    Evaluated { directory: RainPath },
}

impl SourcePath {
    pub fn scope(&self) -> &RainPathScope {
        match self {
            Self::FilePath { path } => path.scope(),
            Self::Evaluated { directory } => directory.scope(),
        }
    }

    pub fn directory(&self) -> Option<RainPath> {
        match self {
            Self::FilePath { path } => path.parent(),
            Self::Evaluated { directory } => Some(directory.clone()),
        }
    }
}

impl std::fmt::Display for SourcePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FilePath { path } => path.fmt(f),
            Self::Evaluated { directory } => f.write_fmt(format_args!("{directory}/<evaluated>")),
        }
    }
}
