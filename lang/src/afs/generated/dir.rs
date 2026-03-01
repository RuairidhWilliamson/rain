use crate::{
    afs::{
        FSEntryTrait,
        area::FileAreaRef,
        entry::FSEntryRef,
        generated::{area::GeneratedFSArea, entry::GeneratedFSEntry},
        path::SealedFilePath,
    },
    driver::{FSEntryQueryResult, FSTrait},
};

#[derive(Debug, Hash, Clone, PartialEq, Eq)]
pub struct GeneratedDir(GeneratedFSEntry);

impl GeneratedDir {
    /// # Safety
    /// Only call this if it is guaranteed the directory exists and is actually a directory (not a symlink or file)
    pub unsafe fn new(ifs: GeneratedFSEntry) -> Self {
        Self(ifs)
    }

    pub fn new_checked(fs: &impl FSTrait, entry: GeneratedFSEntry) -> Option<Self> {
        match fs.query_fs(FSEntryRef::from_generated(&entry)) {
            // Safety: we have just queried the filesystem entry
            Ok(FSEntryQueryResult::Directory) => Some(unsafe { Self::new(entry) }),
            _ => None,
        }
    }

    pub fn root(area: GeneratedFSArea) -> Self {
        Self(GeneratedFSEntry {
            area,
            path: SealedFilePath::root(),
        })
    }

    pub fn fsinner(&self) -> &GeneratedFSEntry {
        &self.0
    }
}

impl FSEntryTrait for GeneratedDir {
    fn area(&self) -> FileAreaRef<'_> {
        self.0.area()
    }

    fn path(&self) -> &SealedFilePath {
        self.0.path()
    }
}

impl std::fmt::Display for GeneratedDir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
