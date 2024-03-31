use std::path::PathBuf;

#[derive(Debug)]
pub struct File {
    pub kind: FileKind,
    pub path: PathBuf,
}

#[derive(Debug)]
pub enum FileKind {
    Source,
    Generated,
    Escaped,
}
