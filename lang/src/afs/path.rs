use super::error::PathError;

/// Represents a UTF-8 absolute path within a file area
///
/// Must start with /
/// Path segments are separated by /
#[derive(Debug, Hash, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FilePath(String);

impl FilePath {
    /// Create a new absolute file path
    ///
    /// If path doesn't start with / it will be prefixed
    pub fn new(path: &str) -> Result<Self, PathError> {
        path_segments(path).try_for_each(valid_path_segment)?;
        Ok(Self(if is_absolute(path) {
            path.to_owned()
        } else {
            String::from("/") + path
        }))
    }

    pub fn root() -> Self {
        Self(String::from("/"))
    }

    pub fn path(&self) -> &str {
        &self.0
    }

    pub fn join(&self, path: &str) -> Result<Self, PathError> {
        let mut s = self.clone();
        s.push(path)?;
        Ok(s)
    }

    pub fn push(&mut self, path: &str) -> Result<(), PathError> {
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

    pub fn parent(&self) -> Option<Self> {
        if self.0.is_empty() {
            return None;
        }
        let p = self.0.rsplit_once('/').map_or("/", |(p, _)| p);
        Some(Self(p.to_owned()))
    }

    pub fn last(&self) -> Option<&str> {
        path_segments(&self.0).last()
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
