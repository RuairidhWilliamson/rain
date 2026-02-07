use std::sync::Arc;

use crate::{
    afs::{
        FSEntryTrait,
        area::FileAreaRef,
        entry::{FSEntry, FSEntryRef},
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
    pub unsafe fn new(entry: FSEntry) -> Self {
        match entry {
            FSEntry::Local(entry) => Self::Local(unsafe { LocalFile::new(entry) } ),
            FSEntry::Generated(entry) => Self::Generated(unsafe { GeneratedFile::new(entry) } ),
        }
    }

    pub fn new_checked(fs: &impl FSTrait, entry: FSEntry) -> Option<Self> {
        match entry {
            FSEntry::Local(entry) => Some(Self::Local(LocalFile::new_checked(fs, entry)? )),
            FSEntry::Generated(entry) => {
                Some(Self::Generated(GeneratedFile::new_checked(fs, entry)? ))
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
