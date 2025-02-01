use std::path::PathBuf;

use crate::afs::{area::FileArea, file::File};

pub trait DriverTrait {
    /// Resolves file path locally returning an absolute path
    fn resolve_file(&self, file: &File) -> PathBuf;

    fn exists(&self, file: &File) -> Result<bool, std::io::Error>;

    fn escape_bin(&self, name: &str) -> Option<PathBuf>;

    fn print(&self, message: String);

    fn extract(&self, file: &File) -> Result<FileArea, Box<dyn std::error::Error>>;

    fn run(&self, area: Option<&FileArea>, bin: &File, args: Vec<String>) -> RunStatus;

    fn download(&self, url: &str) -> File;
}

pub struct RunStatus {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub area: FileArea,
}
