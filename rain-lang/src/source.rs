use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Source {
    pub path: SourcePath,
    pub source: String,
}

impl Source {
    pub fn new(path: &Path) -> Result<Self, std::io::Error> {
        let f = std::fs::File::open(path)?;
        let metadata = f.metadata()?;
        if metadata.is_dir() {
            let new_path = path.join("main.rain");
            tracing::debug!("{path:?} is a directory using {new_path:?}");
            Ok(Self {
                source: std::fs::read_to_string(&new_path)?,
                path: SourcePath::FilePath { path: new_path },
            })
        } else {
            Ok(Self {
                source: std::io::read_to_string(f)?,
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
                directory: std::env::current_dir().unwrap(),
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
            SourcePath::FilePath { path } => path.parent(),
            SourcePath::Evaluated { directory } => Some(directory.as_path()),
        }
    }
}

impl std::fmt::Display for SourcePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourcePath::FilePath { path } => path.display().fmt(f),
            SourcePath::Evaluated { directory } => {
                f.write_fmt(format_args!("{}/<evaluated>", directory.display()))
            }
        }
    }
}
