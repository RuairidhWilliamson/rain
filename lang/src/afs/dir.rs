use std::sync::Arc;

use crate::afs::{
    FSEntryTrait, area::FileAreaRef, generated::dir::GeneratedDir, local::dir::LocalDir,
    path::SealedFilePath,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Dir {
    Generated(Arc<super::generated::dir::GeneratedDir>),
    Local(Arc<super::local::dir::LocalDir>),
}

impl Dir {
    pub fn root(area: FileAreaRef) -> Self {
        match area {
            FileAreaRef::Local(absolute_path_buf) => Self::Local(Arc::new(LocalDir::root(
                Arc::new(absolute_path_buf.clone()),
            ))),
            FileAreaRef::Generated(generated_file_area) => Self::Generated(Arc::new(
                GeneratedDir::root(Arc::new(generated_file_area.clone())),
            )),
        }
    }
}

impl FSEntryTrait for Dir {
    fn area(&self) -> FileAreaRef<'_> {
        match self {
            Dir::Generated(generated_dir) => generated_dir.area(),
            Dir::Local(local_dir) => local_dir.area(),
        }
    }

    fn path(&self) -> &SealedFilePath {
        match self {
            Dir::Generated(generated_dir) => generated_dir.path(),
            Dir::Local(local_dir) => local_dir.path(),
        }
    }
}

impl std::fmt::Display for Dir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Dir::Generated(generated_file) => generated_file.fmt(f),
            Dir::Local(local_file) => local_file.fmt(f),
        }
    }
}
