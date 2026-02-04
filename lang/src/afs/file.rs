use std::sync::Arc;

use crate::afs::{FSEntryTrait, area::FileAreaRef, path::SealedFilePath};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum File {
    Generated(Arc<super::generated::file::GeneratedFile>),
    Local(Arc<super::local::file::LocalFile>),
}

impl FSEntryTrait for File {
    fn area(&self) -> FileAreaRef<'_> {
        match self {
            File::Generated(generated_file) => generated_file.area(),
            File::Local(local_file) => local_file.area(),
        }
    }

    fn path(&self) -> &SealedFilePath {
        match self {
            File::Generated(generated_file) => generated_file.path(),
            File::Local(local_file) => local_file.path(),
        }
    }
}

impl std::fmt::Display for File {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            File::Generated(generated_file) => generated_file.fmt(f),
            File::Local(local_file) => local_file.fmt(f),
        }
    }
}
