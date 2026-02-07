use std::path::Path;

use crate::{
    afs::{
        FSEntryTrait, absolute::AbsolutePathBuf, area::FileAreaRef, entry::FSEntryRef,
        error::PathError, path::SealedFilePath,
    },
    driver::{FSEntryQueryResult, FSTrait},
};

use super::entry::LocalFSEntry;

#[derive(Debug, Hash, Clone, PartialEq, Eq)]
pub struct LocalFile(LocalFSEntry);

impl LocalFile {
    /// # Safety
    /// Only call this if it is guaranteed the file exists and is actually a file (not a symlink or directory)
    pub unsafe fn new(ife: LocalFSEntry) -> Self {
        Self(ife)
    }

    /// Creates a [`GeneratedFile`] by checking it exists
    pub fn new_checked(fs: &impl FSTrait, entry: LocalFSEntry) -> Option<Self> {
        match fs.query_fs(FSEntryRef::from_local(&entry)) {
            // Safety: we have just queried the filesystem entry
            Ok(FSEntryQueryResult::File) => Some(unsafe { Self::new(entry) }),
            _ => None,
        }
    }

    pub fn new_local(path: &Path) -> Result<Self, PathError> {
        let absolute_path = std::path::absolute(path)?;
        let dir = AbsolutePathBuf(
            absolute_path
                .parent()
                .ok_or(PathError::NoParentDirectory)?
                .to_path_buf(),
        );
        let file_name = absolute_path
            .file_name()
            .ok_or(PathError::NoParentDirectory)?
            .to_str()
            .ok_or(PathError::NotUnicode)?;
        Ok(Self(LocalFSEntry {
            area: dir,
            path: SealedFilePath::new(file_name)?,
        }))
    }

    pub fn fsinner(&self) -> &LocalFSEntry {
        &self.0
    }
}

impl FSEntryTrait for LocalFile {
    fn area(&self) -> FileAreaRef<'_> {
        self.0.area()
    }

    fn path(&self) -> &SealedFilePath {
        self.0.path()
    }
}

impl std::fmt::Display for LocalFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
