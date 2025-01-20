use std::path::PathBuf;

use super::file::File;

pub trait FileSystem {
    /// Resolves file path locally returning an absolute path
    fn resolve_file(&self, file: &File) -> PathBuf;

    fn exists(&self, file: &File) -> Result<bool, std::io::Error>;

    fn escape_bin(&self, name: &str) -> Option<PathBuf>;
}
