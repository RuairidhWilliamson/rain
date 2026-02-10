use std::sync::Arc;

use crate::{
    afs::{
        FSEntryTrait,
        area::FileAreaRef,
        entry::{FSEntry, FSEntryRef},
        error::PathError,
        generated::file::GeneratedFile,
        local::file::LocalFile,
        path::SealedFilePath,
    },
    driver::FSTrait,
    runner::value::Value,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum File {
    Generated(GeneratedFile),
    Local(LocalFile),
}

impl File {
    pub fn new_checked(fs: &impl FSTrait, entry: FSEntry) -> Result<Self, PathError> {
        match entry {
            FSEntry::Local(entry) => Ok(Self::Local(LocalFile::new_checked(fs, entry)?)),
            FSEntry::Generated(entry) => {
                Ok(Self::Generated(GeneratedFile::new_checked(fs, entry)?))
            }
        }
    }

    pub fn to_value(self) -> Value {
        match self {
            Self::Generated(file) => Value::GeneratedFile(Arc::new(file)),
            Self::Local(file) => Value::LocalFile(Arc::new(file)),
        }
    }

    pub fn fsinner(&self) -> FSEntryRef<'_> {
        match self {
            Self::Generated(file) => FSEntryRef::Generated(file.fsinner()),
            Self::Local(file) => FSEntryRef::Local(file.fsinner()),
        }
    }
}

impl FSEntryTrait for File {
    fn area(&self) -> FileAreaRef<'_> {
        match self {
            Self::Generated(generated_file) => generated_file.area(),
            Self::Local(local_file) => local_file.area(),
        }
    }

    fn path(&self) -> &SealedFilePath {
        match self {
            Self::Generated(generated_file) => generated_file.path(),
            Self::Local(local_file) => local_file.path(),
        }
    }
}

impl std::fmt::Display for File {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Generated(generated_file) => generated_file.fmt(f),
            Self::Local(local_file) => local_file.fmt(f),
        }
    }
}
