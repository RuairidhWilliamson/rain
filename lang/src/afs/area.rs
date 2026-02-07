use std::sync::Arc;

use crate::runner::value::Value;

use super::absolute::AbsolutePathBuf;

// TODO: Use arcs in FileArea??

/// A file area is a container of files that is not expected to be modified
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum FileArea {
    Local(AbsolutePathBuf),
    Generated(GeneratedFileArea),
}

impl FileArea {
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
}

impl std::fmt::Display for FileArea {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local(path) => f.write_fmt(format_args!("{}", path.0.display())),
            Self::Generated(GeneratedFileArea { id }) => f.write_fmt(format_args!("{id}")),
        }
    }
}

#[derive(Clone, Copy)]
pub enum FileAreaRef<'a> {
    Local(&'a AbsolutePathBuf),
    Generated(&'a GeneratedFileArea),
}

impl FileAreaRef<'_> {
    pub fn to_owned_area(self) -> FileArea {
        match self {
            FileAreaRef::Local(absolute_path_buf) => FileArea::Local(absolute_path_buf.clone()),
            FileAreaRef::Generated(generated_file_area) => {
                FileArea::Generated(generated_file_area.clone())
            }
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

    pub fn to_value(self) -> Value {
        Value::FileArea(Arc::new(FileArea::Generated(self)))
    }
}

impl std::fmt::Display for GeneratedFileArea {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { id } = self;
        f.write_fmt(format_args!("{id}"))
    }
}
