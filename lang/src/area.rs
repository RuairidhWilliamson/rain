use std::{
    fmt::Display,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Hash)]
pub enum FileArea {
    Local(AbsolutePathBuf),
}

#[derive(Debug, Clone, Hash)]
pub struct AbsolutePathBuf(PathBuf);

impl TryFrom<&Path> for AbsolutePathBuf {
    type Error = std::io::Error;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        std::path::absolute(path).map(Self)
    }
}

#[derive(Debug)]
pub enum PathError {
    Dots,
    Backslash,
    NoParentDirectory,
    IOError(std::io::Error),
}

impl Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dots => f.write_str("path cannot contain a segment that only has 3 or more dots"),
            Self::Backslash => f.write_str("path cannot contain backslash"),
            Self::NoParentDirectory => f.write_str("no parent directory"),
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
    area: FileArea,
    path: FilePath,
}

impl File {
    pub fn new(area: FileArea, path: String) -> Result<Self, PathError> {
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
        let path: String = String::from("/") + absolute_path.file_name().unwrap().to_str().unwrap();
        Self::new(FileArea::Local(dir), path)
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

    pub fn resolve(&self) -> PathBuf {
        let FileArea::Local(AbsolutePathBuf(area_path)) = &self.area;
        let FilePath(path) = &self.path;
        let path = path.strip_prefix('/').unwrap();
        area_path.join(path)
    }
}

/// Represents an absolute path in the file area
///
/// Must start with /
/// Path segments are separated by /
#[derive(Debug, Hash, Clone)]
struct FilePath(String);

impl FilePath {
    fn new(path: String) -> Result<FilePath, PathError> {
        if !is_absolute(&path) {
            panic!("path not absolute: {path}")
        }
        path_segments(&path).try_for_each(valid_path_segment)?;
        Ok(Self(path.to_owned()))
    }

    fn push(&mut self, path: &str) -> Result<(), PathError> {
        if is_absolute(path) {
            *self = FilePath::new(path.to_owned())?;
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

    fn parent(&self) -> Option<FilePath> {
        if self.0.is_empty() {
            return None;
        }
        let p = self.0.rsplit_once('/').map(|(p, _)| p).unwrap_or("/");
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
    s.starts_with("/")
}
