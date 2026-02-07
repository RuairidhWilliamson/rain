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
    Generated(Arc<GeneratedDir>),
    Local(Arc<LocalDir>),
}

impl Dir {
    pub unsafe fn new(entry: FSEntry) -> Self {
        match entry {
            FSEntry::Local(entry) => Self::Local(Arc::new(unsafe { LocalDir::new(entry) })),
            FSEntry::Generated(entry) => {
                Self::Generated(Arc::new(unsafe { GeneratedDir::new(entry) }))
            }
        }
    }

    pub fn new_checked(fs: &impl FSTrait, entry: FSEntry) -> Option<Self> {
        match entry {
            FSEntry::Local(entry) => Some(Self::Local(Arc::new(LocalDir::new_checked(fs, entry)?))),
            FSEntry::Generated(entry) => Some(Self::Generated(Arc::new(
                GeneratedDir::new_checked(fs, entry)?,
            ))),
        }
    }

    pub fn root(area: FileAreaRef) -> Self {
        match area {
            FileAreaRef::Local(absolute_path_buf) => {
                Self::Local(Arc::new(LocalDir::root(absolute_path_buf.clone())))
            }
            FileAreaRef::Generated(generated_file_area) => {
                Self::Generated(Arc::new(GeneratedDir::root(generated_file_area.clone())))
            }
        }
    }

    pub fn fsinner(&self) -> FSEntryRef<'_> {
        match self {
            Dir::Generated(generated) => FSEntryRef::Generated(generated.fsinner()),
            Dir::Local(local) => FSEntryRef::Local(local.fsinner()),
        }
    }

    pub fn to_value(self) -> Value {
        match self {
            Dir::Generated(file) => Value::GeneratedDir(file),
            Dir::Local(file) => Value::LocalDir(file),
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
