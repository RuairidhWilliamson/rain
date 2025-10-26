use std::{
    borrow::Cow,
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::{
    afs::{absolute::AbsolutePathBuf, area::FileArea, dir::Dir, entry::FSEntry, file::File},
    runner::{error::RunnerError, internal::InternalFunction},
};

pub trait FSTrait {
    /// Resolves file path locally returning an absolute path
    fn resolve_fs_entry(&self, file: &FSEntry) -> PathBuf;
    fn query_fs(&self, entry: &FSEntry) -> Result<FSEntryQueryResult, std::io::Error>;
}

pub trait DriverTrait: MonitoringTrait + FSTrait {
    fn print(&self, message: String);
    fn escape_bin(&self, name: &str) -> Option<AbsolutePathBuf>;
    fn extract_zip(&self, file: &File) -> Result<FileArea, RunnerError>;
    fn extract_tar_gz(&self, file: &File) -> Result<FileArea, RunnerError>;
    fn extract_tar_xz(&self, file: &File) -> Result<FileArea, RunnerError>;
    fn run(
        &self,
        area: Option<&FileArea>,
        bin: &Path,
        args: Vec<String>,
        options: RunOptions,
    ) -> Result<RunStatus, RunnerError>;
    fn chroot_run(
        &self,
        area: Option<&FileArea>,
        bin: &Path,
        args: Vec<String>,
        options: RunOptions,
    ) -> Result<RunStatus, RunnerError>;
    fn escape_run(
        &self,
        current_dir: &Dir,
        bin: &Path,
        args: Vec<String>,
        options: RunOptions,
    ) -> Result<EscapeRunStatus, RunnerError>;
    fn download(
        &self,
        url: &str,
        outname: &str,
        etag: Option<&[u8]>,
    ) -> Result<DownloadStatus, RunnerError>;
    fn sha256(&self, file: &File) -> Result<String, RunnerError>;
    fn sha512(&self, file: &File) -> Result<String, RunnerError>;
    fn create_area(&self, dirs: &[&FSEntry]) -> Result<FileArea, RunnerError>;
    fn read_file(&self, file: &File) -> Result<String, std::io::Error>;
    fn create_file(&self, contents: &str, name: &str) -> Result<File, RunnerError>;
    fn file_metadata(&self, file: &File) -> Result<FileMetadata, RunnerError>;
    fn glob(&self, dir: &Dir, pattern: &str) -> Result<Vec<File>, RunnerError>;
    fn prelude_src(&self) -> Option<Cow<'static, str>>;
    fn host_triple(&self) -> &str;
    fn export_file(&self, src: &File, dst: &FSEntry) -> Result<(), RunnerError>;
    fn export_dir(&self, src: &Dir, dst: &FSEntry) -> Result<(), RunnerError>;
    fn create_tar(&self, dir: &Dir, name: &str) -> Result<File, RunnerError>;
    fn create_tar_gz(&self, dir: &Dir, name: &str) -> Result<File, RunnerError>;
    fn get_secret(&self, name: &str) -> Result<String, RunnerError>;
    fn git_contents(&self, url: &str, commit: &str) -> Result<FileArea, RunnerError>;
    fn git_lfs_smudge(&self, area: &FileArea) -> Result<FileArea, RunnerError>;
    fn env_var(&self, key: &str) -> Result<Option<String>, RunnerError>;
    fn copy_file(&self, file: &File, name: &str) -> Result<File, RunnerError>;
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

pub struct EscapeRunStatus {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub struct DownloadStatus {
    pub ok: bool,
    pub status_code: Option<u16>,
    pub file: Option<File>,
    pub etag: Option<Vec<u8>>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum FSEntryQueryResult {
    File,
    Directory,
    Symlink,
    NotExist,
}

impl std::fmt::Display for FSEntryQueryResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::File => f.write_str("is file"),
            Self::Directory => f.write_str("is directory"),
            Self::Symlink => f.write_str("is symlink"),
            Self::NotExist => f.write_str("does not exist"),
        }
    }
}

pub struct FileMetadata {
    pub size: u64,
}
