use crate::{
    afs::{FSEntryTrait, area::FileAreaRef, entry::FSEntryRef, path::SealedFilePath},
    driver::{FSEntryQueryResult, FSTrait},
};

use super::entry::GeneratedFSEntry;

#[derive(Debug, Hash, Clone, PartialEq, Eq)]
pub struct GeneratedFile(GeneratedFSEntry);

impl GeneratedFile {
    /// # Safety
    /// Only call this if it is guaranteed the file exists and is actually a file (not a symlink or directory)
    pub unsafe fn new(ife: GeneratedFSEntry) -> Self {
        Self(ife)
    }

    /// Creates a [`GeneratedFile`] by checking it exists
    pub fn new_checked(fs: &impl FSTrait, entry: GeneratedFSEntry) -> Option<Self> {
        match fs.query_fs(FSEntryRef::from_generated(&entry)) {
            // Safety: we have just queried the filesystem entry
            Ok(FSEntryQueryResult::File) => Some(unsafe { Self::new(entry) }),
            _ => None,
        }
    }

    pub fn fsinner(&self) -> &GeneratedFSEntry {
        &self.0
    }
}

impl FSEntryTrait for GeneratedFile {
    fn area(&self) -> FileAreaRef<'_> {
        self.0.area()
    }

    fn path(&self) -> &SealedFilePath {
        self.0.path()
    }
}

impl std::fmt::Display for GeneratedFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
