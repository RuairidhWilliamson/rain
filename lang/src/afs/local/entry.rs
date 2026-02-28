use termcolor::{Color, ColorSpec, WriteColor};

use crate::afs::{
    FSEntryTrait, absolute::AbsolutePathBuf, area::FileAreaRef, path::SealedFilePath,
};

#[derive(
    Debug, Hash, PartialOrd, Ord, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize,
)]
pub struct LocalFSEntry {
    pub area: AbsolutePathBuf,
    pub path: SealedFilePath,
}

impl LocalFSEntry {
    pub fn new(area: AbsolutePathBuf, path: SealedFilePath) -> Self {
        Self { area, path }
    }

    pub fn write_color(&self, writer: &mut impl WriteColor) -> std::io::Result<()> {
        writer.set_color(ColorSpec::new().set_fg(Some(Color::White)))?;
        write!(writer, "({})", self.area)?;
        writer.reset()?;
        write!(writer, "{}", self.path.path())?;
        Ok(())
    }
}

impl std::fmt::Display for LocalFSEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("({}){}", self.area, self.path.path()))
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
