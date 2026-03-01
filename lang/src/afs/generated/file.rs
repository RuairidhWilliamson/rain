use crate::{
    afs::{
        FSEntryTrait, area::FileAreaRef, entry::FSEntryRef, error::PathError,
        generated::entry::GeneratedFSEntry, path::SealedFilePath,
    },
    driver::{FSEntryQueryResult, FSTrait},
};

#[derive(Debug, Hash, Clone, PartialEq, Eq)]
pub struct GeneratedFile(GeneratedFSEntry);

impl GeneratedFile {
    /// # Safety
    /// Only call this if it is guaranteed the file exists and is actually a file (not a symlink or directory)
    pub unsafe fn new(ife: GeneratedFSEntry) -> Self {
        Self(ife)
    }

    /// Creates a [`GeneratedFile`] by checking it exists
    pub fn new_checked(fs: &impl FSTrait, entry: GeneratedFSEntry) -> Result<Self, PathError> {
        match fs.query_fs(FSEntryRef::from_generated(&entry))? {
            // Safety: we have just queried the filesystem entry
            FSEntryQueryResult::File => Ok(unsafe { Self::new(entry) }),
            _ => Err(PathError::FileNotExist),
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
