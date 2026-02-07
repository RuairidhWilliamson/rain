use std::sync::Arc;

use crate::{
    afs::{
        FSEntryTrait, absolute::AbsolutePathBuf, area::FileAreaRef, entry::FSEntryRef,
        path::SealedFilePath,
    },
    driver::{FSEntryQueryResult, FSTrait},
};

use super::entry::LocalFSEntry;

#[derive(Debug, Hash, Clone, PartialEq, Eq)]
pub struct LocalDir(LocalFSEntry);

impl LocalDir {
    /// # Safety
    /// Only call this if it is guaranteed the directory exists and is actually a directory (not a symlink or file)
    pub unsafe fn new(ifs: LocalFSEntry) -> Self {
        Self(ifs)
    }

    pub fn new_checked(fs: &impl FSTrait, entry: LocalFSEntry) -> Option<Self> {
        match fs.query_fs(FSEntryRef::from_local(&entry)) {
            // Safety: we have just queried the filesystem entry
            Ok(FSEntryQueryResult::Directory) => Some(unsafe { Self::new(entry) }),
            _ => None,
        }
    }

    pub fn root(area: Arc<AbsolutePathBuf>) -> Self {
        Self(LocalFSEntry {
            area,
            path: SealedFilePath::root(),
        })
    }

    pub fn fsinner(&self) -> &LocalFSEntry {
        &self.0
    }
}

impl FSEntryTrait for LocalDir {
    fn area(&self) -> FileAreaRef<'_> {
        self.0.area()
    }

    fn path(&self) -> &SealedFilePath {
        self.0.path()
    }
}

impl std::fmt::Display for LocalDir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
