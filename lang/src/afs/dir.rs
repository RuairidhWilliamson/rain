use std::sync::Arc;

use crate::{
    afs::{
        FSEntryTrait,
        area::FileAreaRef,
        entry::{FSEntry, FSEntryRef},
        generated::dir::GeneratedDir,
        local::dir::LocalDir,
        path::SealedFilePath,
    },
    driver::FSTrait,
    runner::value::Value,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Dir {
    Generated(GeneratedDir),
    Local(LocalDir),
}

impl Dir {
    pub unsafe fn new(entry: FSEntry) -> Self {
        match entry {
            FSEntry::Local(entry) => Self::Local((unsafe { LocalDir::new(entry) })),
            FSEntry::Generated(entry) => Self::Generated((unsafe { GeneratedDir::new(entry) })),
        }
    }

    pub fn new_checked(fs: &impl FSTrait, entry: FSEntry) -> Option<Self> {
        match entry {
            FSEntry::Local(entry) => Some(Self::Local((LocalDir::new_checked(fs, entry)?))),
            FSEntry::Generated(entry) => {
                Some(Self::Generated((GeneratedDir::new_checked(fs, entry)?)))
            }
        }
    }

    pub fn root(area: FileAreaRef) -> Self {
        match area {
            FileAreaRef::Local(absolute_path_buf) => {
                Self::Local((LocalDir::root(absolute_path_buf.clone())))
            }
            FileAreaRef::Generated(generated_file_area) => {
                Self::Generated((GeneratedDir::root(generated_file_area.clone())))
            }
        }
    }

    pub fn fsinner(&self) -> FSEntryRef<'_> {
        match self {
            Self::Generated(generated) => FSEntryRef::Generated(generated.fsinner()),
            Self::Local(local) => FSEntryRef::Local(local.fsinner()),
        }
    }

    pub fn to_value(self) -> Value {
        match self {
            Self::Generated(file) => Value::GeneratedDir(Arc::new(file)),
            Self::Local(file) => Value::LocalDir(Arc::new(file)),
        }
    }
}

impl FSEntryTrait for Dir {
    fn area(&self) -> FileAreaRef<'_> {
        match self {
            Self::Generated(generated_dir) => generated_dir.area(),
            Self::Local(local_dir) => local_dir.area(),
        }
    }

    fn path(&self) -> &SealedFilePath {
        match self {
            Self::Generated(generated_dir) => generated_dir.path(),
            Self::Local(local_dir) => local_dir.path(),
        }
    }
}

impl std::fmt::Display for Dir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Generated(generated_file) => generated_file.fmt(f),
            Self::Local(local_file) => local_file.fmt(f),
        }
    }
}
