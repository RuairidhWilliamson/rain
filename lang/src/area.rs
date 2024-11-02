use std::{
    error::Error,
    fmt::Display,
    path::{Path, PathBuf},
};

use crate::config::Config;

#[derive(Debug, Clone, Hash)]
pub enum FileArea {
    Empty,
    Local(AbsolutePathBuf),
    Generated(GeneratedFileArea),
}

impl FileArea {
    fn path(&self, config: &Config) -> Option<PathBuf> {
        match self {
            Self::Empty => None,
            Self::Local(p) => Some(p.to_path_buf()),
            Self::Generated(GeneratedFileArea { id }) => {
                Some(config.base_generated_dir.join(id.to_string()))
            }
        }
    }
}

impl std::fmt::Display for FileArea {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileArea::Empty => f.write_str("<empty>"),
            FileArea::Local(path) => f.write_fmt(format_args!("<{}>", path.0.display())),
            FileArea::Generated(GeneratedFileArea { id }) => f.write_fmt(format_args!("<{id}>")),
        }
    }
}

#[derive(Debug, Clone, Hash)]
pub struct GeneratedFileArea {
    id: uuid::Uuid,
}

impl GeneratedFileArea {
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
        }
    }
}

#[derive(Debug, Clone, Hash)]
pub struct AbsolutePathBuf(PathBuf);

impl TryFrom<&Path> for AbsolutePathBuf {
    type Error = std::io::Error;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        std::path::absolute(path).map(Self)
    }
}

impl std::ops::Deref for AbsolutePathBuf {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.0.as_path()
    }
}

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

#[derive(Debug, Hash, Clone)]
pub struct File {
    pub area: FileArea,
    path: FilePath,
}

impl File {
    pub fn new(area: FileArea, path: &str) -> Result<Self, PathError> {
        Ok(Self {
            area,
            path: FilePath::new(path)?,
        })
    }

    pub fn new_local(path: &Path) -> Result<Self, PathError> {
        let absolute_path = std::path::absolute(path)?;
        let dir = AbsolutePathBuf(
            absolute_path
                .parent()
                .ok_or(PathError::NoParentDirectory)?
                .to_path_buf(),
        );
        let file_name = absolute_path
            .file_name()
            .ok_or(PathError::NoParentDirectory)?
            .to_str()
            .ok_or(PathError::NotUnicode)?;
        let path: String = String::from("/") + file_name;
        Self::new(FileArea::Local(dir), &path)
    }

    pub fn push(&mut self, path: &str) -> Result<(), PathError> {
        self.path.push(path)
    }

    pub fn join(&self, path: &str) -> Result<Self, PathError> {
        let mut s = self.clone();
        s.push(path)?;
        Ok(s)
    }

    pub fn parent(&self) -> Option<Self> {
        Some(Self {
            area: self.area.clone(),
            path: self.path.parent()?,
        })
    }

    pub fn resolve(&self, config: &Config) -> PathBuf {
        let area_path = self.area.path(config).unwrap();
        let FilePath(path) = &self.path;
        let Some(path) = path.strip_prefix('/') else {
            unreachable!("file path must start with /");
        };
        area_path.join(path)
    }
}

impl std::fmt::Display for File {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}{}", &self.area, &self.path.0))
    }
}

/// Represents an absolute path in the file area
///
/// Must start with /
/// Path segments are separated by /
#[derive(Debug, Hash, Clone)]
struct FilePath(String);

impl FilePath {
    fn new(path: &str) -> Result<Self, PathError> {
        path_segments(path).try_for_each(valid_path_segment)?;
        Ok(Self(if is_absolute(path) {
            path.to_owned()
        } else {
            String::from("/") + path
        }))
    }

    fn push(&mut self, path: &str) -> Result<(), PathError> {
        if is_absolute(path) {
            *self = Self::new(path)?;
            return Ok(());
        }
        // Resolve any dot segments
        let mut pop_count = 0;
        let mut segments: Vec<&str> = Vec::new();
        for s in path_segments(path) {
            match s {
                "." => {}
                ".." => {
                    if segments.pop().is_none() {
                        pop_count += 1;
                    }
                }
                _ if s.chars().all(|c| c == '.') => {
                    return Err(PathError::Dots);
                }
                _ => {
                    segments.push(s);
                }
            }
        }
        let existing_n = path_segments(&self.0).count();
        if existing_n < pop_count {
            return Err(PathError::NoParentDirectory);
        }
        self.0 = path_segments(&self.0)
            .take(existing_n - pop_count)
            .chain(segments)
            .flat_map(|s| ["/", s])
            .collect();
        Ok(())
    }

    fn parent(&self) -> Option<Self> {
        if self.0.is_empty() {
            return None;
        }
        let p = self.0.rsplit_once('/').map_or("/", |(p, _)| p);
        Some(Self(p.to_owned()))
    }
}

fn path_segments(s: &str) -> impl DoubleEndedIterator<Item = &'_ str> {
    s.split('/').filter(|s| !s.is_empty())
}

fn valid_path_segment(s: &str) -> Result<(), PathError> {
    if s.is_empty() {
        return Ok(());
    }
    if s.contains('\\') {
        return Err(PathError::Backslash);
    }
    if s.chars().all(|c| c == '.') {
        return Err(PathError::Dots);
    }
    Ok(())
}

fn is_absolute(s: &str) -> bool {
    s.starts_with('/')
}
