use std::path::Path;

use crate::driver::{FSEntryQueryResult, FSTrait};

use super::{
    absolute::AbsolutePathBuf,
    area::FileArea,
    entry::{FSEntry, FSEntryTrait},
    error::PathError,
    path::SealedFilePath,
};

#[derive(Debug, Hash, Clone, PartialEq, Eq)]
pub struct File(FSEntry);

impl File {
    /// # Safety
    /// Only call this if it is guaranteed the file exists and is actually a file (not a symlink or directory)
    pub unsafe fn new(ife: FSEntry) -> Self {
        Self(ife)
    }

    /// Creates a [`File`] by checking it exists
    pub fn new_checked(fs: &impl FSTrait, entry: FSEntry) -> Option<Self> {
        match fs.query_fs(&entry) {
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
        Ok(Self(FSEntry {
            area: FileArea::Local(dir),
            path: SealedFilePath::new(file_name)?,
        }))
    }
}

impl FSEntryTrait for File {
    fn inner(&self) -> &FSEntry {
        &self.0
    }
}

impl std::fmt::Display for File {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
