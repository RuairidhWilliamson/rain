use std::sync::Arc;

use crate::afs::{
    FSEntryTrait, area::FileArea, generated::entry::GeneratedFSEntry, local::entry::LocalFSEntry,
    path::SealedFilePath,
};

#[derive(Debug, Hash, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FSEntry {
    Local(LocalFSEntry),
    Generated(GeneratedFSEntry),
}

impl FSEntry {
    pub fn new(area: FileArea, path: SealedFilePath) -> Self {
        match area {
            FileArea::Local(area) => Self::Local(LocalFSEntry::new(Arc::new(area), path)),
            FileArea::Generated(area) => {
                Self::Generated(GeneratedFSEntry::new(Arc::new(area), path))
            }
        }
    }

    pub fn as_fs_entry_ref(&self) -> FSEntryRef<'_> {
        match self {
            FSEntry::Local(entry) => FSEntryRef::Local(entry),
            FSEntry::Generated(entry) => FSEntryRef::Generated(entry),
        }
    }
}

impl From<LocalFSEntry> for FSEntry {
    fn from(entry: LocalFSEntry) -> Self {
        Self::Local(entry)
    }
}

impl From<GeneratedFSEntry> for FSEntry {
    fn from(entry: GeneratedFSEntry) -> Self {
        Self::Generated(entry)
    }
}

impl std::fmt::Display for FSEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Generated(generated_file) => generated_file.fmt(f),
            Self::Local(local_file) => local_file.fmt(f),
        }
    }
}

#[derive(Clone, Copy)]
pub enum FSEntryRef<'a> {
    Local(&'a LocalFSEntry),
    Generated(&'a GeneratedFSEntry),
}

impl<'a> FSEntryRef<'a> {
    pub fn from_local(entry: &'a LocalFSEntry) -> FSEntryRef<'a> {
        FSEntryRef::Local(entry)
    }

    pub fn from_generated(entry: &'a GeneratedFSEntry) -> FSEntryRef<'a> {
        FSEntryRef::Generated(entry)
    }
}

impl<'a> From<&'a LocalFSEntry> for FSEntryRef<'a> {
    fn from(entry: &'a LocalFSEntry) -> Self {
        Self::from_local(entry)
    }
}

impl<'a> From<&'a GeneratedFSEntry> for FSEntryRef<'a> {
    fn from(entry: &'a GeneratedFSEntry) -> Self {
        Self::from_generated(entry)
    }
}

impl FSEntryTrait for FSEntryRef<'_> {
    fn area(&self) -> super::area::FileAreaRef<'_> {
        match self {
            FSEntryRef::Local(inner) => inner.area(),
            FSEntryRef::Generated(inner) => inner.area(),
        }
    }

    fn path(&self) -> &super::path::SealedFilePath {
        match self {
            FSEntryRef::Local(inner) => inner.path(),
            FSEntryRef::Generated(inner) => inner.path(),
        }
    }
}
