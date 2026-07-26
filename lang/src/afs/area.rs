use std::sync::Arc;

use crate::{
    afs::{generated::area::GeneratedFSArea, local::area::LocalFSArea},
    runner::value::Value,
};

/// A FS area is a container of files and directories
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum FSArea {
    Local(LocalFSArea),
    Generated(GeneratedFSArea),
}

impl FSArea {
    pub fn is_local(&self) -> bool {
        match self {
            Self::Local(_) => true,
            Self::Generated(_) => false,
        }
    }

    pub fn as_area_ref(&self) -> FileAreaRef<'_> {
        match self {
            Self::Local(absolute_path_buf) => FileAreaRef::Local(absolute_path_buf),
            Self::Generated(generated_file_area) => FileAreaRef::Generated(generated_file_area),
        }
    }

    pub fn to_value(self) -> Value {
        match self {
            Self::Local(area) => Value::LocalFSArea(Arc::new(area)),
            Self::Generated(area) => area.to_value(),
        }
    }
}

impl From<LocalFSArea> for FSArea {
    fn from(area: LocalFSArea) -> Self {
        Self::Local(area)
    }
}

impl From<GeneratedFSArea> for FSArea {
    fn from(area: GeneratedFSArea) -> Self {
        Self::Generated(area)
    }
}

impl std::fmt::Display for FSArea {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local(path) => f.write_fmt(format_args!("{}", path.0.display())),
            Self::Generated(GeneratedFSArea {
                id,
                git_describe: _,
            }) => f.write_fmt(format_args!("{id}")),
        }
    }
}

#[derive(Clone, Copy)]
pub enum FileAreaRef<'a> {
    Local(&'a LocalFSArea),
    Generated(&'a GeneratedFSArea),
}

impl FileAreaRef<'_> {
    pub fn to_owned_area(self) -> FSArea {
        match self {
            FileAreaRef::Local(area) => FSArea::Local(area.clone()),
            FileAreaRef::Generated(area) => FSArea::Generated(area.clone()),
        }
    }

    pub fn is_local(&self) -> bool {
        matches!(self, Self::Local(_))
    }
}

impl<'a> From<&'a LocalFSArea> for FileAreaRef<'a> {
    fn from(area: &'a LocalFSArea) -> Self {
        Self::Local(area)
    }
}

impl<'a> From<&'a GeneratedFSArea> for FileAreaRef<'a> {
    fn from(area: &'a GeneratedFSArea) -> Self {
        Self::Generated(area)
    }
}
