use std::{collections::HashMap, path::PathBuf};

use crate::{
    afs::{area::FileArea, file::File},
    runner::{error::RunnerError, internal::InternalFunction},
};

pub trait DriverTrait: MonitoringTrait {
    /// Resolves file path locally returning an absolute path
    fn resolve_file(&self, file: &File) -> PathBuf;
    fn exists(&self, file: &File) -> Result<bool, std::io::Error>;
    fn print(&self, message: String);
    fn escape_bin(&self, name: &str) -> Option<PathBuf>;
    fn extract_zip(&self, file: &File) -> Result<FileArea, RunnerError>;
    fn extract_tar_gz(&self, file: &File) -> Result<FileArea, RunnerError>;
    fn extract_tar_xz(&self, file: &File) -> Result<FileArea, RunnerError>;
    fn run(
        &self,
        area: Option<&FileArea>,
        bin: &File,
        args: Vec<String>,
        options: RunOptions,
    ) -> Result<RunStatus, RunnerError>;
    fn download(&self, url: &str, outname: &str) -> Result<DownloadStatus, RunnerError>;
    fn sha256(&self, file: &File) -> Result<String, RunnerError>;
    fn merge_dirs(&self, dirs: &[&File]) -> Result<FileArea, RunnerError>;
    fn read_file(&self, file: &File) -> Result<String, std::io::Error>;
    fn write_file(&self, contents: &str, name: &str) -> Result<File, RunnerError>;
}

pub trait MonitoringTrait {
    fn enter_call(&self, _s: &str) {}
    fn exit_call(&self, _s: &str) {}
    fn enter_internal_call(&self, _f: &InternalFunction) {}
    fn exit_internal_call(&self, _f: &InternalFunction) {}
}

pub struct RunOptions {
    pub inherit_env: bool,
    pub env: HashMap<String, String>,
}

pub struct RunStatus {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub area: FileArea,
    pub stdout: String,
    pub stderr: String,
}

pub struct DownloadStatus {
    pub ok: bool,
    pub status_code: Option<u16>,
    pub file: Option<File>,
}
