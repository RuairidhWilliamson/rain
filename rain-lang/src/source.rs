use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

#[derive(Debug, Clone)]
pub struct Source {
    pub path: SourcePath,
    pub source: Rc<str>,
}

impl Source {
    pub fn new(path: &Path) -> Result<Self, std::io::Error> {
        let f = std::fs::File::open(path)?;
        let metadata = f.metadata()?;
        if metadata.is_dir() {
            let new_path = path.join("main.rain");
            tracing::debug!("{path:?} is a directory using {new_path:?}");
            let source = std::fs::read_to_string(&new_path)?;
            Ok(Self {
                source: source.into(),
                path: SourcePath::FilePath { path: new_path },
            })
        } else {
            let source = std::io::read_to_string(f)?;
            Ok(Self {
                source: source.into(),
                path: SourcePath::FilePath {
                    path: path.to_path_buf(),
                },
            })
        }
    }
}

impl From<&str> for Source {
    fn from(source: &str) -> Self {
        Self {
            path: SourcePath::Evaluated {
                directory: std::env::current_dir().expect("get current directory"),
            },
            source: source.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum SourcePath {
    FilePath { path: PathBuf },
    Evaluated { directory: PathBuf },
}

impl SourcePath {
    pub fn directory(&self) -> Option<&Path> {
        match self {
            Self::FilePath { path } => path.parent(),
            Self::Evaluated { directory } => Some(directory.as_path()),
        }
    }
}

impl std::fmt::Display for SourcePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FilePath { path } => path.display().fmt(f),
            Self::Evaluated { directory } => {
                f.write_fmt(format_args!("{}/<evaluated>", directory.display()))
            }
        }
    }
}
