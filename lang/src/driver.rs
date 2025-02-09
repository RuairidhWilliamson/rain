use std::path::PathBuf;

use crate::{
    afs::{area::FileArea, file::File},
    runner::{error::RunnerError, internal::InternalFunction},
};

pub trait DriverTrait {
    /// Resolves file path locally returning an absolute path
    fn resolve_file(&self, file: &File) -> PathBuf;
    fn exists(&self, file: &File) -> Result<bool, std::io::Error>;
    fn escape_bin(&self, name: &str) -> Option<PathBuf>;
    fn print(&self, message: String);
    fn extract(&self, file: &File) -> Result<FileArea, RunnerError>;
    fn run(
        &self,
        area: Option<&FileArea>,
        bin: &File,
        args: Vec<String>,
    ) -> Result<RunStatus, RunnerError>;
    fn download(&self, url: &str) -> Result<DownloadStatus, RunnerError>;
    fn enter_call(&self, _s: &str) {}
    fn exit_call(&self, _s: &str) {}
    fn enter_internal_call(&self, _f: &InternalFunction) {}
    fn exit_internal_call(&self, _f: &InternalFunction) {}
}

pub struct RunStatus {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub area: FileArea,
}

pub struct DownloadStatus {
    pub ok: bool,
    pub status_code: Option<u16>,
    pub file: Option<File>,
}
