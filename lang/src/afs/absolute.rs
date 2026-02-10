use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct AbsolutePathBuf(pub PathBuf);

impl TryFrom<&Path> for AbsolutePathBuf {
    type Error = std::io::Error;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        std::path::absolute(path).map(Self)
    }
}

impl TryFrom<PathBuf> for AbsolutePathBuf {
    type Error = std::io::Error;

    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        std::path::absolute(path).map(Self)
    }
}

impl std::ops::Deref for AbsolutePathBuf {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.0.as_path()
    }
}

impl std::fmt::Display for AbsolutePathBuf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.0.display()))
    }
}
