pub mod absolute;
pub mod area;
pub mod dir;
pub mod entry;
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
}
