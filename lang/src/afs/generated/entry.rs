use crate::afs::{
    FSEntryTrait, area::FileAreaRef, generated::area::GeneratedFileArea, path::SealedFilePath,
};

#[derive(Debug, Hash, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GeneratedFSEntry {
    pub area: GeneratedFileArea,
    pub path: SealedFilePath,
}

impl GeneratedFSEntry {
    pub fn new(area: GeneratedFileArea, path: SealedFilePath) -> Self {
        Self { area, path }
    }
}

impl std::fmt::Display for GeneratedFSEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}{}", self.area, self.path.path()))
    }
}

impl FSEntryTrait for GeneratedFSEntry {
    fn area(&self) -> FileAreaRef<'_> {
        FileAreaRef::Generated(&self.area)
    }

    fn path(&self) -> &SealedFilePath {
        &self.path
    }
}
