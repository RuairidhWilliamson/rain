use super::{area::FileArea, path::FilePath};

pub trait FSEntryTrait {
    fn inner(&self) -> &FSEntry;

    fn area(&self) -> &FileArea {
        &self.inner().area
    }

    fn path(&self) -> &FilePath {
        &self.inner().path
    }
}

#[derive(Debug, Hash, Clone, PartialEq, Eq)]
pub struct FSEntry {
    pub area: FileArea,
    pub path: FilePath,
}

impl FSEntry {
    pub fn new(area: FileArea, path: FilePath) -> Self {
        Self { area, path }
    }
}

impl std::fmt::Display for FSEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("<{}>{}", self.area, self.path.path()))
    }
}
