use crate::afs::{
    FSEntryTrait, absolute::AbsolutePathBuf, area::FileAreaRef, path::SealedFilePath,
};

#[derive(Debug, Hash, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LocalFSEntry {
    pub area: AbsolutePathBuf,
    pub path: SealedFilePath,
}

impl LocalFSEntry {
    pub fn new(area: AbsolutePathBuf, path: SealedFilePath) -> Self {
        Self { area, path }
    }
}

impl std::fmt::Display for LocalFSEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:?}{}", self.area, self.path.path()))
    }
}

impl FSEntryTrait for LocalFSEntry {
    fn area(&self) -> FileAreaRef<'_> {
        FileAreaRef::Local(&self.area)
    }

    fn path(&self) -> &SealedFilePath {
        &self.path
    }
}
