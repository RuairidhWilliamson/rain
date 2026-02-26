use std::path::Path;

use crate::{
    afs::{
        FSEntryTrait, absolute::AbsolutePathBuf, area::FileAreaRef, entry::FSEntryRef,
        error::PathError, path::SealedFilePath,
    },
    driver::FSTrait,
    hash::FileHash,
};

use super::entry::LocalFSEntry;

#[derive(Debug, Hash, Clone, PartialEq, Eq)]
pub struct LocalFile {
    entry: LocalFSEntry,
    hash: FileHash,
}

impl LocalFile {
    /// # Safety
    /// Only call this if it is guaranteed the file exists and is actually a file (not a symlink or directory)
    pub unsafe fn new(entry: LocalFSEntry, hash: FileHash) -> Self {
        Self { entry, hash }
    }

    /// Creates a [`GeneratedFile`] by checking it exists
    pub fn new_checked(fs: &impl FSTrait, entry: LocalFSEntry) -> Result<Self, PathError> {
        let hash = fs.query_file_hash(FSEntryRef::from_local(&entry))?;
        // Safety: we have just queried the filesystem entry
        Ok(unsafe { Self::new(entry, hash) })
    }

    pub fn new_local(fs: &impl FSTrait, path: &Path) -> Result<Self, PathError> {
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
        Self::new_checked(
            fs,
            LocalFSEntry {
                area: dir,
                path: SealedFilePath::new(file_name)?,
            },
        )
    }

    pub fn fsinner(&self) -> &LocalFSEntry {
        &self.entry
    }

    pub fn file_hash(&self) -> &FileHash {
        &self.hash
    }
}

impl FSEntryTrait for LocalFile {
    fn area(&self) -> FileAreaRef<'_> {
        self.entry.area()
    }

    fn path(&self) -> &SealedFilePath {
        self.entry.path()
    }
}

impl std::fmt::Display for LocalFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.entry.fmt(f)
    }
}
