use super::absolute::AbsolutePathBuf;

/// A file area is a container of files that is not expected to be modified
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum FileArea {
    Local(AbsolutePathBuf),
    Generated(GeneratedFileArea),
}

impl FileArea {
    pub fn is_local(&self) -> bool {
        match self {
            FileArea::Local(_) => true,
            FileArea::Generated(_) => false,
        }
    }
}

impl std::fmt::Display for FileArea {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local(path) => f.write_fmt(format_args!("{}", path.0.display())),
            Self::Generated(GeneratedFileArea { id }) => f.write_fmt(format_args!("{id}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct GeneratedFileArea {
    pub id: uuid::Uuid,
}

impl Default for GeneratedFileArea {
    fn default() -> Self {
        Self::new()
    }
}

impl GeneratedFileArea {
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
        }
    }
}
