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
    Generated(Arc<GeneratedFile>),
    Local(Arc<LocalFile>),
}

impl File {
    pub unsafe fn new(entry: FSEntry) -> Self {
        match entry {
            FSEntry::Local(entry) => Self::Local(Arc::new(unsafe { LocalFile::new(entry) })),
            FSEntry::Generated(entry) => {
                Self::Generated(Arc::new(unsafe { GeneratedFile::new(entry) }))
            }
        }
    }

    pub fn new_checked(fs: &impl FSTrait, entry: FSEntry) -> Option<Self> {
        match entry {
            FSEntry::Local(entry) => {
                Some(Self::Local(Arc::new(LocalFile::new_checked(fs, entry)?)))
            }
            FSEntry::Generated(entry) => Some(Self::Generated(Arc::new(
                GeneratedFile::new_checked(fs, entry)?,
            ))),
        }
    }

    pub fn to_value(self) -> Value {
        match self {
            File::Generated(file) => Value::GeneratedFile(file),
            File::Local(file) => Value::LocalFile(file),
        }
    }

    pub fn fsinner(&self) -> FSEntryRef<'_> {
        match self {
            File::Generated(file) => FSEntryRef::Generated(file.fsinner()),
            File::Local(file) => FSEntryRef::Local(file.fsinner()),
        }
    }
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
