pub mod absolute;
pub mod area;
pub mod dir;
pub mod error;
pub mod file;
pub mod generated;
pub mod local;
pub mod path;

use crate::afs::area::FileAreaRef;

pub use dir::Dir;
pub use file::File;

pub trait FSEntryTrait {
    fn area(&self) -> FileAreaRef<'_>;
    fn path(&self) -> &path::SealedFilePath;

    fn fsinner(&self) -> FSEntryRef<'_> {
        FSEntryRef {
            area: self.area(),
            path: self.path(),
        }
    }
}

pub struct FSEntryRef<'a> {
    pub area: FileAreaRef<'a>,
    pub path: &'a path::SealedFilePath,
}
