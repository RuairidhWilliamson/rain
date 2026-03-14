use std::{
    borrow::Cow,
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use git2::{Cred, Oid};
use poison_panic::MutexExt as _;
use rain_lang::{
    afs::{
        FSEntryTrait as _, File,
        absolute::AbsolutePathBuf,
        area::{FSArea, FileAreaRef},
        dir::Dir,
        entry::{FSEntry, FSEntryRef},
        generated::{
            area::GeneratedFSArea, dir::GeneratedDir, entry::GeneratedFSEntry, file::GeneratedFile,
        },
        path::SealedFilePath,
    },
    driver::{
        CreateAreaOptions, DownloadStatus, DriverTrait, EscapeRunStatus, FSEntryQueryResult,
        FSTrait, FileMetadata, PathConflicts, RunOptions, RunStatus,
        monitoring::{Call, MonitoringTrait},
    },
    hash::FileHash,
    runner::error::RunnerError,
};

use sha2::Digest as _;

use crate::config::Config;

pub type PrintHandler<'a> = Box<dyn Fn(&str) + 'a + Send>;
pub type CallHandler<'a> = Box<dyn Fn(&Call) + 'a + Send>;

pub struct DriverImpl<'a> {
    pub config: Config,
    pub custom_config: HashMap<String, Arc<String>>,
    pub prints: Mutex<Vec<String>>,
    pub print_handler: Option<PrintHandler<'a>>,
    pub enter_handler: Option<CallHandler<'a>>,
    pub exit_handler: Option<CallHandler<'a>>,
    pub embed: Option<Cow<'static, str>>,
    pub host_triple: Cow<'static, str>,
    pub internal_counts: Mutex<HashMap<Arc<String>, usize>>,
}

pub const fn default_host_triple() -> &'static str {
    env!("TARGET_PLATFORM")
}

impl DriverImpl<'_> {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            custom_config: HashMap::default(),
            prints: Mutex::default(),
            print_handler: None,
            enter_handler: None,
            exit_handler: None,
            embed: Some(include_str!("../../lib/embed/embed.rain").into()),
            host_triple: default_host_triple().into(),
            internal_counts: Default::default(),
        }
    }

    pub fn get_counter(&self, name: &Arc<String>) -> usize {
        let guard = self.internal_counts.plock();
        guard.get(name).copied().unwrap_or_default()
    }

    fn create_empty_area(&self) -> Result<GeneratedFSArea, RunnerError> {
        self.create_overlay_area(std::iter::empty(), &CreateAreaOptions::default())
    }

    pub fn create_overlay_area<'a>(
        &self,
        fs_entries: impl Iterator<Item = FSEntryRef<'a>>,
        options: &CreateAreaOptions,
    ) -> Result<GeneratedFSArea, RunnerError> {
        let area = GeneratedFSArea::new();
        let output_dir = GeneratedDir::root(area.clone());
        let output_dir_path = self.resolve_fs_entry(FSEntryRef::Generated(output_dir.fsinner()));
        if matches!(std::fs::exists(&output_dir_path), Ok(true)) {
            return Err(RunnerError::Makeshift(
                "output directory already exists".into(),
            ));
        }
        std::fs::create_dir_all(&output_dir_path)
            .map_err(|err| RunnerError::MakeshiftIO("create_dir_all".into(), err))?;
        for fs_entry in fs_entries {
            let path = self.resolve_fs_entry(fs_entry);
            let metadata = std::fs::metadata(&path)
                .map_err(|err| RunnerError::MakeshiftIO("metadata".into(), err))?;
            if metadata.is_file() {
                let rel_dest = path
                    .file_name()
                    .ok_or_else(|| RunnerError::Makeshift("strip prefix failed".into()))?;
                let dest_path = output_dir_path.join(rel_dest);
                match options.conflicts {
                    PathConflicts::Throw => {
                        if std::fs::exists(&dest_path).map_err(|err| {
                            RunnerError::MakeshiftIO("check file exists".into(), err)
                        })? {
                            return Err(RunnerError::ConflictingFileNames(PathBuf::from(rel_dest)));
                        }
                    }
                    PathConflicts::Overwrite => {}
                }
                std::fs::copy(path, dest_path)
                    .map_err(|err| RunnerError::MakeshiftIO("copy file".into(), err))?;
            } else if metadata.is_dir() {
                let walker = ignore::WalkBuilder::new(&path)
                    .hidden(!options.include_hidden)
                    .build();
                let dir_name = path
                    .file_name()
                    .ok_or_else(|| RunnerError::Makeshift("no dir name".into()))?;
                for entry in walker {
                    let Ok(entry) = entry else {
                        continue;
                    };
                    let Some(file_type) = entry.file_type() else {
                        continue;
                    };
                    if file_type.is_file() {
                        let rel_dest = entry
                            .path()
                            .strip_prefix(&path)
                            .map_err(|_| RunnerError::Makeshift("strip prefix failed".into()))?;
                        let dest_path = if options.flatten_input_dirs {
                            output_dir_path.join(rel_dest)
                        } else {
                            output_dir_path.join(dir_name).join(rel_dest)
                        };
                        std::fs::create_dir_all(dest_path.parent().ok_or_else(|| {
                            RunnerError::Makeshift("parent does not exist".into())
                        })?)
                        .map_err(|err| RunnerError::MakeshiftIO("create parent dir".into(), err))?;
                        match options.conflicts {
                            PathConflicts::Throw => {
                                if std::fs::exists(&dest_path).map_err(|err| {
                                    RunnerError::MakeshiftIO("check file exists".into(), err)
                                })? {
                                    return Err(RunnerError::ConflictingFileNames(PathBuf::from(
                                        rel_dest,
                                    )));
                                }
                            }
                            PathConflicts::Overwrite => {}
                        }
                        std::fs::copy(entry.path(), dest_path)
                            .map_err(|err| RunnerError::MakeshiftIO("copy file".into(), err))?;
                    }
                }
            } else {
                return Err(RunnerError::Makeshift(
                    "unexpected, not a file or dir".into(),
                ));
            }
        }
        Ok(area)
    }
}

impl FSTrait for DriverImpl<'_> {
    fn resolve_fs_entry(&self, entry: FSEntryRef) -> PathBuf {
        self.config.resolve_fs_entry(entry)
    }

    fn query_fs(&self, entry: FSEntryRef) -> Result<FSEntryQueryResult, std::io::Error> {
        self.config.query_fs(entry)
    }

    fn query_file_hash(&self, entry: FSEntryRef) -> Result<FileHash, std::io::Error> {
        self.config.query_file_hash(entry)
    }
}

impl DriverTrait for DriverImpl<'_> {
    fn increment_counter(&self, name: Arc<String>) {
        let mut guard = self.internal_counts.plock();
        let entry = guard.entry(Arc::clone(&name)).or_default();
        *entry += 1;
    }

    #[cfg(target_family = "unix")]
    fn escape_bin(&self, name: &str) -> Option<AbsolutePathBuf> {
        // Unix separates path values using colons
        const PATH_SEPARATOR: u8 = b':';
        use std::os::unix::ffi::OsStrExt as _;

        std::env::var_os("PATH")?
            .as_bytes()
            .split(|&b| b == PATH_SEPARATOR)
            .find_map(|p| find_bin_in_dir(Path::new(std::ffi::OsStr::from_bytes(p)), name))
    }

    #[cfg(target_family = "windows")]
    fn escape_bin(&self, name: &str) -> Option<AbsolutePathBuf> {
        // Windows separates path values using semi colons
        const PATH_SEPARATOR: u16 = {
            let mut out = [0u16; 1];
            ';'.encode_utf16(&mut out);
            out[0]
        };
        use std::os::windows::ffi::{OsStrExt as _, OsStringExt as _};
        std::env::var_os("PATH")?
            .encode_wide()
            .collect::<Vec<u16>>()
            .split(|&b| b == PATH_SEPARATOR)
            .find_map(|p| {
                let p = std::ffi::OsString::from_wide(p);
                find_bin_in_dir(Path::new(&p), name)
            })
    }

    fn print(&self, message: String) {
        if let Some(ph) = &self.print_handler {
            ph(&message);
        }
        self.prints.plock().push(message);
    }

    fn extract_zip(&self, file: &File) -> Result<GeneratedFSArea, RunnerError> {
        let resolved_path = self.resolve_fs_entry(file.fsinner());
        let area = self.create_empty_area()?;
        let output_dir = GeneratedDir::root(area.clone());
        let output_dir_path = self.resolve_fs_entry(output_dir.fsinner().into());
        log::debug!("extract zip {resolved_path:?}");
        let f = std::fs::File::open(resolved_path).map_err(RunnerError::AreaIOError)?;
        let mut zip = zip::read::ZipArchive::new(f)
            .map_err(|err| RunnerError::ExtractError(Box::new(err)))?;
        for i in 0..zip.len() {
            let mut zip_file = zip
                .by_index(i)
                .map_err(|err| RunnerError::ExtractError(Box::new(err)))?;
            let Some(name) = zip_file.enclosed_name() else {
                continue;
            };
            if !zip_file.is_file() {
                continue;
            }
            let path = output_dir_path.join(name);
            std::fs::create_dir_all(
                path.parent()
                    .ok_or(RunnerError::Makeshift("zip path no parent".into()))?,
            )
            .map_err(RunnerError::AreaIOError)?;
            let mut opts = std::fs::OpenOptions::new();
            opts.create(true).truncate(false).write(true);

            // We want to fallback to create all files with rwx so that binaries can be executed
            #[cfg(target_family = "unix")]
            std::os::unix::fs::OpenOptionsExt::mode(
                &mut opts,
                0o770,
                // This doesn't work but should eventually be replaced with
                // `zip_file.unix_mode().unwrap_or(0o770)`
            );

            let mut out = opts.open(path).map_err(RunnerError::AreaIOError)?;
            std::io::copy(&mut zip_file, &mut out).map_err(RunnerError::AreaIOError)?;
        }
        Ok(area)
    }

    fn extract_gzip(&self, file: &File, name: &str) -> Result<GeneratedFile, RunnerError> {
        let resolved_path = self.resolve_fs_entry(file.fsinner());
        let f = std::fs::File::open(resolved_path).map_err(RunnerError::AreaIOError)?;
        let mut raw = flate2::read::GzDecoder::new(f);
        let area = self.create_empty_area()?;
        let path = SealedFilePath::new(name)?;
        let entry = GeneratedFSEntry::new(area, path);
        let resolved_path = self.resolve_fs_entry((&entry).into());
        let mut out_file =
            std::fs::File::create_new(resolved_path).map_err(RunnerError::AreaIOError)?;
        std::io::copy(&mut raw, &mut out_file).map_err(RunnerError::AreaIOError)?;
        // Safety: We just created the file
        let file = unsafe { GeneratedFile::new(entry) };
        Ok(file)
    }

    fn extract_xz(&self, file: &File, name: &str) -> Result<GeneratedFile, RunnerError> {
        let resolved_path = self.resolve_fs_entry(file.fsinner());
        let f = std::fs::File::open(resolved_path).map_err(RunnerError::AreaIOError)?;
        let mut raw = liblzma::read::XzDecoder::new(f);
        let area = self.create_empty_area()?;
        let path = SealedFilePath::new(name)?;
        let entry = GeneratedFSEntry::new(area, path);
        let resolved_path = self.resolve_fs_entry((&entry).into());
        let mut out_file =
            std::fs::File::create_new(resolved_path).map_err(RunnerError::AreaIOError)?;
        std::io::copy(&mut raw, &mut out_file).map_err(RunnerError::AreaIOError)?;
        // Safety: We just created the file
        let file = unsafe { GeneratedFile::new(entry) };
        Ok(file)
    }

    fn extract_tar(&self, file: &File) -> Result<GeneratedFSArea, RunnerError> {
        let resolved_path = self.resolve_fs_entry(file.fsinner());
        let area = self.create_empty_area()?;
        let output_dir = GeneratedDir::root(area.clone());
        let output_dir_path = self.resolve_fs_entry(output_dir.fsinner().into());
        let f = std::fs::File::open(resolved_path).map_err(RunnerError::AreaIOError)?;
        let mut archive = tar::Archive::new(f);
        archive
            .unpack(output_dir_path)
            .map_err(|err| RunnerError::ExtractError(Box::new(err)))?;
        Ok(area)
    }

    fn run(
        &self,
        overlay_area: Option<FileAreaRef>,
        bin: &Path,
        args: Vec<String>,
        RunOptions { inherit_env, env }: RunOptions,
    ) -> Result<RunStatus, RunnerError> {
        let output_area = if let Some(overlay_area) = overlay_area {
            self.create_overlay_area(
                std::iter::once(Dir::root(overlay_area).fsinner()),
                &CreateAreaOptions {
                    include_hidden: true,
                    flatten_input_dirs: true,
                    ..Default::default()
                },
            )?
        } else {
            self.create_empty_area()?
        };
        let output_dir = GeneratedDir::root(output_area.clone());
        let output_dir_path = self.resolve_fs_entry(output_dir.fsinner().into());
        let mut cmd = std::process::Command::new(bin);
        cmd.current_dir(output_dir_path);
        cmd.args(args);
        if !inherit_env {
            cmd.env_clear();
        }
        cmd.envs(env);
        log::debug!("Running {cmd:?}");
        let output = match cmd.output() {
            Ok(output) => output,
            Err(err) => {
                return Ok(RunStatus {
                    success: false,
                    exit_code: None,
                    area: output_area,
                    stdout: String::new(),
                    stderr: err.to_string(),
                });
            }
        };
        let success = output.status.success();
        let exit_code = output.status.code();
        Ok(RunStatus {
            success,
            exit_code,
            area: output_area,
            stdout: String::from_utf8(output.stdout)?,
            stderr: String::from_utf8(output.stderr)?,
        })
    }

    fn escape_run(
        &self,
        current_dir: &Dir,
        bin: &Path,
        args: Vec<String>,
        RunOptions { inherit_env, env }: RunOptions,
    ) -> Result<EscapeRunStatus, RunnerError> {
        let current_dir_path = self.resolve_fs_entry(current_dir.fsinner());
        let mut cmd = std::process::Command::new(bin);
        cmd.current_dir(current_dir_path);
        cmd.args(args);
        if !inherit_env {
            cmd.env_clear();
        }
        cmd.envs(env);
        log::debug!("Running {cmd:?}");
        let output = match cmd.output() {
            Ok(output) => output,
            Err(err) => {
                return Ok(EscapeRunStatus {
                    success: false,
                    exit_code: None,
                    stdout: String::new(),
                    stderr: err.to_string(),
                });
            }
        };
        let success = output.status.success();
        let exit_code = output.status.code();
        Ok(EscapeRunStatus {
            success,
            exit_code,
            stdout: String::from_utf8(output.stdout)?,
            stderr: String::from_utf8(output.stderr)?,
        })
    }

    fn download(
        &self,
        url: &str,
        name: &str,
        etag: Option<&[u8]>,
    ) -> Result<DownloadStatus, RunnerError> {
        let agent = ureq::Agent::new_with_config(
            ureq::config::Config::builder()
                .http_status_as_error(false)
                .build(),
        );
        let mut request = agent.get(url);
        if let Some(etag) = etag {
            request = request.header(ureq::http::header::IF_NONE_MATCH, etag);
        }
        log::debug!("Download {url}");
        let mut response = request
            .call()
            .map_err(|err| RunnerError::MakeshiftIO("download request".into(), err.into_io()))?;
        log::debug!("Download complete {url} {}", response.status());
        let etag: Option<Vec<u8>> = response
            .headers()
            .get(ureq::http::header::ETAG)
            .map(|h| h.as_bytes().to_vec());
        let area = self.create_empty_area()?;
        let path = SealedFilePath::new(name)?;
        let entry = GeneratedFSEntry::new(area, path);
        let output_path = self.resolve_fs_entry((&entry).into());
        let mut out = std::fs::File::create_new(output_path)
            .map_err(|err| RunnerError::MakeshiftIO("create download file".into(), err))?;
        let body = response.body_mut();
        std::io::copy(&mut body.as_reader(), &mut out)
            .map_err(|err| RunnerError::MakeshiftIO("download file".into(), err))?;
        // Safety: We just created the file and checked for errors so it is present
        let output = unsafe { GeneratedFile::new(entry) };
        Ok(DownloadStatus {
            ok: response.status().is_success(),
            status_code: Some(response.status().as_u16()),
            file: Some(output),
            etag,
        })
    }

    fn sha256(&self, file: &File) -> Result<[u8; 32], RunnerError> {
        let resolved_path = self.resolve_fs_entry(file.fsinner());
        let mut file = std::fs::File::open(resolved_path).map_err(RunnerError::AreaIOError)?;
        let mut hasher = sha2::Sha256::new();
        std::io::copy(&mut file, &mut hasher).map_err(RunnerError::AreaIOError)?;
        let hash_result = hasher.finalize();
        Ok(hash_result.into())
    }

    fn sha512(&self, file: &File) -> Result<[u8; 64], RunnerError> {
        let resolved_path = self.resolve_fs_entry(file.fsinner());
        let mut file = std::fs::File::open(resolved_path).map_err(RunnerError::AreaIOError)?;
        let mut hasher = sha2::Sha512::new();
        std::io::copy(&mut file, &mut hasher).map_err(RunnerError::AreaIOError)?;
        let hash_result = hasher.finalize();
        Ok(hash_result.into())
    }

    fn create_area(
        &self,
        dirs: &[FSEntryRef],
        options: &CreateAreaOptions,
    ) -> Result<GeneratedFSArea, RunnerError> {
        self.create_overlay_area(dirs.iter().copied(), options)
    }

    fn read_file(&self, file: &File) -> Result<String, std::io::Error> {
        let resolved_path = self.resolve_fs_entry(file.fsinner());
        let contents = std::fs::read_to_string(resolved_path)?;
        Ok(contents)
    }

    fn create_file(
        &self,
        contents: &[u8],
        name: &str,
        executable: bool,
    ) -> Result<GeneratedFile, RunnerError> {
        let area = self.create_empty_area()?;
        let path = SealedFilePath::new(name)?;
        let entry = GeneratedFSEntry::new(area, path);
        let resolved_path = self.resolve_fs_entry((&entry).into());
        std::fs::write(&resolved_path, contents).map_err(RunnerError::AreaIOError)?;
        // Setting executable is only supported on unix
        #[cfg(target_family = "unix")]
        {
            if executable {
                use std::os::unix::fs::PermissionsExt as _;

                let metadata =
                    std::fs::metadata(&resolved_path).map_err(RunnerError::AreaIOError)?;
                let mut permissions = metadata.permissions();
                // Set execute for owner
                permissions.set_mode(permissions.mode() | 0o100);
                std::fs::set_permissions(resolved_path, permissions)
                    .map_err(RunnerError::AreaIOError)?;
            }
        }
        // Safety: We just created the file
        let file = unsafe { GeneratedFile::new(entry) };
        Ok(file)
    }

    fn file_metadata(&self, file: &File) -> Result<FileMetadata, RunnerError> {
        let metadata = std::fs::metadata(self.resolve_fs_entry(file.fsinner()))
            .map_err(RunnerError::AreaIOError)?;
        Ok(FileMetadata {
            size: metadata.len(),
        })
    }

    #[expect(clippy::unwrap_used)]
    fn glob(&self, dir: &Dir, _pattern: &str) -> Result<Vec<File>, RunnerError> {
        let base_path = self.resolve_fs_entry(dir.fsinner());
        // TODO: Implement proper globbing instead of ignoring the pattern
        let mut out = Vec::new();
        for entry in ignore::Walk::new(&base_path) {
            let entry = entry.unwrap();
            let file_type = entry.file_type().unwrap();
            if file_type.is_symlink() || file_type.is_dir() {
                continue;
            }
            if file_type.is_file() {
                let p = entry.path();
                let p = p.strip_prefix(&base_path).unwrap().to_str().unwrap();
                let p = dir.fsinner().path().join(p).unwrap();
                let file =
                    File::new_checked(self, FSEntry::new(dir.area().to_owned_area(), p)).unwrap();
                out.push(file);
            }
        }
        Ok(out)
    }

    fn embed_src(&self) -> Option<Cow<'static, str>> {
        self.embed.clone()
    }

    fn host_triple(&self) -> &str {
        &self.host_triple
    }

    fn export_file(&self, src: &File, dst: FSEntryRef<'_>) -> Result<(), RunnerError> {
        let src_path = self.resolve_fs_entry(src.fsinner());
        let dst_path = self.resolve_fs_entry(dst);
        // TODO: Backup old file before overwriting, if it exists
        std::fs::copy(src_path, dst_path)
            .map_err(|err| RunnerError::MakeshiftIO("copy file".into(), err))?;
        Ok(())
    }

    fn export_dir(&self, src: &Dir, dst: FSEntryRef<'_>) -> Result<(), RunnerError> {
        let src_path = self.resolve_fs_entry(src.fsinner());
        let dst_path = self.resolve_fs_entry(dst);
        // TODO: Backup old file before overwriting, if it exists
        let walker = ignore::WalkBuilder::new(&src_path).build();
        for entry in walker {
            let Ok(entry) = entry else {
                continue;
            };
            let Some(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_file() {
                let rel_dest = entry
                    .path()
                    .strip_prefix(&src_path)
                    .map_err(|_| RunnerError::Makeshift("strip prefix failed".into()))?;
                let dest_entry = dst_path.join(rel_dest);
                std::fs::create_dir_all(
                    dest_entry
                        .parent()
                        .ok_or_else(|| RunnerError::Makeshift("parent does not exist".into()))?,
                )
                .map_err(|err| RunnerError::MakeshiftIO("create parent dir".into(), err))?;
                std::fs::copy(entry.path(), dest_entry)
                    .map_err(|err| RunnerError::MakeshiftIO("copy file".into(), err))?;
            }
        }
        Ok(())
    }

    fn create_tar(&self, dir: &Dir, name: &str) -> Result<GeneratedFile, RunnerError> {
        let dir_path = self.resolve_fs_entry(dir.fsinner());
        let area = self.create_empty_area()?;
        let path = SealedFilePath::new(name)?;
        let entry = GeneratedFSEntry::new(area, path);
        let output_path = self.resolve_fs_entry((&entry).into());
        let f = std::fs::File::create(output_path).map_err(RunnerError::AreaIOError)?;
        let mut archive = tar::Builder::new(f);
        archive
            .append_dir_all(".", dir_path)
            .map_err(|err| RunnerError::MakeshiftIO("create tar".into(), err))?;
        archive
            .finish()
            .map_err(|err| RunnerError::MakeshiftIO("create tar flush".into(), err))?;
        // Safety: We just created the file
        let file = unsafe { GeneratedFile::new(entry) };
        Ok(file)
    }

    fn compress_gzip(&self, file: &File, name: &str) -> Result<GeneratedFile, RunnerError> {
        let area = self.create_empty_area()?;
        let path = SealedFilePath::new(name)?;
        let entry = GeneratedFSEntry::new(area, path);
        let output_path = self.resolve_fs_entry((&entry).into());
        let f = std::fs::File::create(output_path).map_err(RunnerError::AreaIOError)?;
        let mut encoder = flate2::write::GzEncoder::new(f, flate2::Compression::default());
        let mut read = std::fs::File::open(self.resolve_fs_entry(file.fsinner()))
            .map_err(RunnerError::AreaIOError)?;
        std::io::copy(&mut read, &mut encoder).map_err(RunnerError::AreaIOError)?;
        encoder.finish().map_err(RunnerError::AreaIOError)?;
        // Safety: We just created the file
        let file = unsafe { GeneratedFile::new(entry) };
        Ok(file)
    }

    fn compress_zstd(
        &self,
        file: &File,
        name: &str,
        level: u8,
    ) -> Result<GeneratedFile, RunnerError> {
        let area = self.create_empty_area()?;
        let path = SealedFilePath::new(name)?;
        let entry = GeneratedFSEntry::new(area, path);
        let output_path = self.resolve_fs_entry((&entry).into());
        let f = std::fs::File::create(output_path).map_err(RunnerError::AreaIOError)?;
        let mut encoder =
            zstd::Encoder::new(f, i32::from(level)).map_err(RunnerError::AreaIOError)?;
        let mut read = std::fs::File::open(self.resolve_fs_entry(file.fsinner()))
            .map_err(RunnerError::AreaIOError)?;
        std::io::copy(&mut read, &mut encoder).map_err(RunnerError::AreaIOError)?;
        encoder.finish().map_err(RunnerError::AreaIOError)?;
        // Safety: We just created the file
        let file = unsafe { GeneratedFile::new(entry) };
        Ok(file)
    }

    fn extract_zstd(&self, file: &File, name: &str) -> Result<GeneratedFile, RunnerError> {
        let resolved_path = self.resolve_fs_entry(file.fsinner());
        let f = std::fs::File::open(resolved_path).map_err(RunnerError::AreaIOError)?;
        let mut raw = zstd::Decoder::new(f).map_err(RunnerError::AreaIOError)?;
        let area = self.create_empty_area()?;
        let path = SealedFilePath::new(name)?;
        let entry = GeneratedFSEntry::new(area, path);
        let resolved_path = self.resolve_fs_entry((&entry).into());
        let mut out_file =
            std::fs::File::create_new(resolved_path).map_err(RunnerError::AreaIOError)?;
        std::io::copy(&mut raw, &mut out_file).map_err(RunnerError::AreaIOError)?;
        // Safety: We just created the file
        let file = unsafe { GeneratedFile::new(entry) };
        Ok(file)
    }

    fn get_secret(&self, name: &str) -> Result<String, RunnerError> {
        // TODO: Ask before accessing
        match std::env::var(name) {
            Ok(secret) => return Ok(secret),
            Err(std::env::VarError::NotPresent) => {}
            Err(std::env::VarError::NotUnicode(_)) => {
                return Err(RunnerError::Makeshift("secret not utf8".into()));
            }
        }
        match std::fs::read_to_string("secrets.toml") {
            Ok(secrets_contents) => {
                let toml: toml::Value = toml::from_str(&secrets_contents)
                    .map_err(|_err| RunnerError::Makeshift("parse secrets.toml".into()))?;
                return Ok(toml
                    .get(name)
                    .ok_or_else(|| RunnerError::Makeshift("secret is not present".into()))?
                    .as_str()
                    .ok_or_else(|| RunnerError::Makeshift("secret is not a string".into()))?
                    .to_owned());
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(RunnerError::MakeshiftIO("open secrets.toml".into(), err)),
        }
        Err(RunnerError::Makeshift(
            format!("secret {name:?} not found").into(),
        ))
    }

    fn git_contents(&self, url: &str, commit: &str) -> Result<GeneratedFSArea, RunnerError> {
        let area = self.create_empty_area()?;
        let dir = GeneratedDir::root(area);
        let commit = Oid::from_str(commit)
            .map_err(|err| RunnerError::Makeshift(format!("parse commit hash: {err}").into()))?;
        let mut fo = git2::FetchOptions::new();
        let mut rcb = git2::RemoteCallbacks::new();
        rcb.credentials(|_url, username_from_url, allowed_types| {
            let username = username_from_url.unwrap_or("git");
            if allowed_types.contains(git2::CredentialType::USERNAME) {
                return git2::Cred::username(username);
            }
            Cred::ssh_key_from_agent(username)
        });
        fo.remote_callbacks(rcb);
        let repo = git2::build::RepoBuilder::new()
            .fetch_options(fo)
            .with_checkout(git2::build::CheckoutBuilder::new())
            .clone(url, &self.resolve_fs_entry(dir.fsinner().into()))
            .map_err(|err| RunnerError::Makeshift(format!("clone repo: {err}").into()))?;
        repo.set_head_detached(commit)
            .map_err(|err| RunnerError::Makeshift(format!("set head: {err}").into()))?;
        Ok(dir.fsinner().area.clone())
    }

    fn git_lfs_smudge(&self, _area: &FSArea) -> Result<GeneratedFSArea, RunnerError> {
        // let dir = Dir::root(area.clone());
        // let path = self.resolve_fs_entry(dir.inner());
        // for entry in ignore::Walk::new(&path) {
        //     let entry = entry.unwrap();
        //     if !entry.file_type().unwrap().is_file() {
        //         continue;
        //     }
        //     let Ok(lfs_entry) = git_lfs_rs::Pointer::from_path(entry.path()) else {
        //         continue;
        //     };
        //     dbg!(lfs_entry);
        // }

        Err(RunnerError::Makeshift(
            "git lfs smudge unimplemented".into(),
        ))
    }

    fn env_var(&self, key: &str) -> Result<Option<String>, RunnerError> {
        Ok(std::env::var(key).ok())
    }

    fn copy_file(
        &self,
        file: &File,
        name: &str,
        executable: bool,
    ) -> Result<GeneratedFile, RunnerError> {
        let area = self.create_empty_area()?;
        let path = SealedFilePath::new(name)?;
        let entry = GeneratedFSEntry::new(area, path);
        let input_path = self.resolve_fs_entry(file.fsinner());
        let output_path = self.resolve_fs_entry((&entry).into());
        // OPTIMISE: Can use hardlink instead
        std::fs::copy(input_path, &output_path).map_err(RunnerError::AreaIOError)?;
        // Setting executable is only supported on unix
        #[cfg(target_family = "unix")]
        {
            if executable {
                use std::os::unix::fs::PermissionsExt as _;

                let metadata = std::fs::metadata(&output_path).map_err(RunnerError::AreaIOError)?;
                let mut permissions = metadata.permissions();
                // Set execute for owner
                permissions.set_mode(permissions.mode() | 0o100);
                std::fs::set_permissions(output_path, permissions)
                    .map_err(RunnerError::AreaIOError)?;
            }
        }
        // Safety: We just created it
        Ok(unsafe { GeneratedFile::new(entry) })
    }

    fn copy_dir(
        &self,
        dir: &Dir,
        name: &str,
        include_hidden: bool,
    ) -> Result<GeneratedDir, RunnerError> {
        let area = self.create_empty_area()?;
        let path = SealedFilePath::new(name)?;
        let entry = GeneratedFSEntry::new(area, path);
        let input_path = self.resolve_fs_entry(dir.fsinner());
        let output_path = self.resolve_fs_entry((&entry).into());
        let walker = ignore::WalkBuilder::new(&input_path)
            .hidden(!include_hidden)
            .build();
        for entry in walker {
            let Ok(entry) = entry else {
                continue;
            };
            let Some(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_file() {
                let rel_dest = entry
                    .path()
                    .strip_prefix(&input_path)
                    .map_err(|_| RunnerError::Makeshift("strip prefix failed".into()))?;
                let dest_entry = output_path.join(name).join(rel_dest);
                std::fs::create_dir_all(
                    dest_entry
                        .parent()
                        .ok_or_else(|| RunnerError::Makeshift("parent does not exist".into()))?,
                )
                .map_err(|err| RunnerError::MakeshiftIO("create parent dir".into(), err))?;
                std::fs::copy(entry.path(), dest_entry)
                    .map_err(|err| RunnerError::MakeshiftIO("copy file".into(), err))?;
            }
        }
        // Safety: We just created it
        Ok(unsafe { GeneratedDir::new(entry) })
    }

    fn config(&self, name: &str) -> Option<Arc<String>> {
        Some(Arc::clone(self.custom_config.get(name)?))
    }
}

impl MonitoringTrait for DriverImpl<'_> {
    fn enter_call(&self, s: &Call) {
        if let Some(ph) = &self.enter_handler {
            ph(s);
        }
    }

    fn exit_call(&self, s: &Call) {
        if let Some(ph) = &self.exit_handler {
            ph(s);
        }
    }
}

#[cfg(target_family = "unix")]
fn find_bin_in_dir(dir: &Path, name: &str) -> Option<AbsolutePathBuf> {
    std::fs::read_dir(dir).ok()?.find_map(|e| {
        let p = e.ok()?;
        if p.file_name().to_str()? == name {
            Some(AbsolutePathBuf::try_from(p.path()).ok()?)
        } else {
            None
        }
    })
}

#[cfg(target_family = "windows")]
fn find_bin_in_dir(dir: &Path, name: &str) -> Option<AbsolutePathBuf> {
    std::fs::read_dir(dir).ok()?.find_map(|e| {
        let entry = e.ok()?;
        let path = entry.path();
        // Only recognise .exe files, this is not correct because .cmd and .bat files should also match
        if path.extension()?.to_str()? != "exe" {
            return None;
        }
        let filestem = path.file_stem()?.to_str()?;
        if filestem == name {
            Some(AbsolutePathBuf::try_from(path).ok()?)
        } else {
            None
        }
    })
}
