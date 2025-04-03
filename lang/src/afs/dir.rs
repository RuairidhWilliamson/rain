use super::{
    area::FileArea,
    entry::{FSEntry, FSEntryTrait},
    path::FilePath,
};

#[derive(Debug, Hash, Clone, PartialEq, Eq)]
pub struct Dir(FSEntry);

impl Dir {
    /// # Safety
    /// Only call this if it is guaranteed the directory exists and is actually a directory (not a symlink or file)
    pub unsafe fn new(ifs: FSEntry) -> Self {
        Self(ifs)
    }

    pub fn root(area: FileArea) -> Self {
        Self(FSEntry {
            area,
            path: FilePath::root(),
        })
    }
}

impl FSEntryTrait for Dir {
    fn inner(&self) -> &FSEntry {
        &self.0
    }
}

impl std::fmt::Display for Dir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("<{}>{}", self.0.area, self.0.path.path()))
    }
}
