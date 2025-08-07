use super::{area::FileArea, path::SealedFilePath};

pub trait FSEntryTrait {
    fn inner(&self) -> &FSEntry;

    fn area(&self) -> &FileArea {
        &self.inner().area
    }

    fn path(&self) -> &SealedFilePath {
        &self.inner().path
    }
}

#[derive(Debug, Hash, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FSEntry {
    // TODO: Make this Arc<FileArea> so we don't have an expensive clone
    pub area: FileArea,
    pub path: SealedFilePath,
}

impl FSEntry {
    pub fn new(area: FileArea, path: SealedFilePath) -> Self {
        Self { area, path }
    }
}

impl std::fmt::Display for FSEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}{}", self.area, self.path.path()))
    }
}
