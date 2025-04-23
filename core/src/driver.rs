use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use poison_panic::MutexExt as _;
use rain_lang::{
    afs::{
        area::{FileArea, GeneratedFileArea},
        dir::Dir,
        entry::{FSEntry, FSEntryTrait as _},
        file::File,
        path::FilePath,
    },
    driver::{
        DownloadStatus, DriverTrait, FSEntryQueryResult, FSTrait, FileMetadata, MonitoringTrait,
        RunOptions, RunStatus,
    },
    runner::{error::RunnerError, internal::InternalFunction},
};
use sha2::Digest as _;

use crate::config::Config;

pub type PrintHandler<'a> = Box<dyn Fn(&str) + 'a>;

pub struct DriverImpl<'a> {
    pub config: Config,
    pub prints: Mutex<Vec<String>>,
    pub print_handler: Option<PrintHandler<'a>>,
    pub enter_handler: Option<PrintHandler<'a>>,
    pub exit_handler: Option<PrintHandler<'a>>,
}

impl DriverImpl<'_> {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            prints: Mutex::default(),
            print_handler: None,
            enter_handler: None,
            exit_handler: None,
        }
    }

    fn create_empty_area(&self) -> Result<FileArea, RunnerError> {
        let area = FileArea::Generated(GeneratedFileArea::new());
        let output_dir = Dir::root(area.clone());
        let output_dir_path = self.resolve_fs_entry(output_dir.inner());
        std::fs::create_dir_all(&output_dir_path).map_err(RunnerError::AreaIOError)?;
        Ok(area)
    }

    fn create_overlay_area<'a>(
        &self,
        overlay_dirs: impl Iterator<Item = &'a Dir>,
    ) -> Result<FileArea, RunnerError> {
        let area = FileArea::Generated(GeneratedFileArea::new());
        let output_dir = Dir::root(area.clone());
        let output_dir_path = self.resolve_fs_entry(output_dir.inner());
        std::fs::create_dir_all(&output_dir_path).map_err(RunnerError::AreaIOError)?;
        for dir in overlay_dirs {
            let dir_path = self.resolve_fs_entry(dir.inner());
            dircpy::copy_dir(dir_path, &output_dir_path).map_err(RunnerError::AreaIOError)?;
        }
        Ok(area)
    }
}

impl FSTrait for DriverImpl<'_> {
    fn resolve_fs_entry(&self, entry: &FSEntry) -> PathBuf {
        self.config.resolve_fs_entry(entry)
    }

    fn query_fs(&self, entry: &FSEntry) -> Result<FSEntryQueryResult, std::io::Error> {
        self.config.query_fs(entry)
    }
}

impl DriverTrait for DriverImpl<'_> {
    #[expect(clippy::unwrap_used)]
    fn escape_bin(&self, name: &str) -> Option<PathBuf> {
        std::env::var_os("PATH")?
            .into_string()
            .unwrap()
            .split(PATH_SEPARATOR)
            .find_map(|p| find_bin_in_dir(Path::new(p), name))
    }

    fn print(&self, message: String) {
        if let Some(ph) = &self.print_handler {
            ph(&message);
        }
        self.prints.plock().push(message);
    }

    fn extract_zip(&self, file: &File) -> Result<FileArea, RunnerError> {
        let resolved_path = self.resolve_fs_entry(file.inner());
        let area = self.create_empty_area()?;
        let output_dir = Dir::root(area.clone());
        let output_dir_path = self.resolve_fs_entry(output_dir.inner());
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
                zip_file.unix_mode().unwrap_or(0o770),
            );

            let mut out = opts.open(path).map_err(RunnerError::AreaIOError)?;
            std::io::copy(&mut zip_file, &mut out).map_err(RunnerError::AreaIOError)?;
        }
        Ok(area)
    }

    fn extract_tar_gz(&self, file: &File) -> Result<FileArea, RunnerError> {
        let resolved_path = self.resolve_fs_entry(file.inner());
        let area = self.create_empty_area()?;
        let output_dir = Dir::root(area.clone());
        let output_dir_path = self.resolve_fs_entry(output_dir.inner());
        let f = std::fs::File::open(resolved_path).map_err(RunnerError::AreaIOError)?;
        let raw_tar = flate2::read::GzDecoder::new(f);
        let mut archive = tar::Archive::new(raw_tar);
        archive
            .unpack(output_dir_path)
            .map_err(|err| RunnerError::ExtractError(Box::new(err)))?;
        Ok(area)
    }

    fn extract_tar_xz(&self, file: &File) -> Result<FileArea, RunnerError> {
        let resolved_path = self.resolve_fs_entry(file.inner());
        let area = self.create_empty_area()?;
        let output_dir = Dir::root(area.clone());
        let output_dir_path = self.resolve_fs_entry(output_dir.inner());
        let f = std::fs::File::open(resolved_path).map_err(RunnerError::AreaIOError)?;
        let raw_tar = liblzma::read::XzDecoder::new(f);
        let mut archive = tar::Archive::new(raw_tar);
        archive
            .unpack(output_dir_path)
            .map_err(|err| RunnerError::ExtractError(Box::new(err)))?;
        Ok(area)
    }

    #[expect(clippy::unwrap_used)]
    fn run(
        &self,
        overlay_area: Option<&FileArea>,
        bin: &File,
        args: Vec<String>,
        RunOptions { inherit_env, env }: RunOptions,
    ) -> Result<RunStatus, RunnerError> {
        let output_area = if let Some(overlay_area) = overlay_area {
            self.create_overlay_area(std::iter::once(&Dir::root(overlay_area.clone())))?
        } else {
            self.create_empty_area()?
        };
        let output_dir = Dir::root(output_area.clone());
        let output_dir_path = self.resolve_fs_entry(output_dir.inner());
        let bin_file = self.resolve_fs_entry(bin.inner());
        let mut cmd = std::process::Command::new(bin_file);
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
            stdout: String::from_utf8(output.stdout).unwrap(),
            stderr: String::from_utf8(output.stderr).unwrap(),
        })
    }

    #[expect(clippy::unwrap_used)]
    fn download(
        &self,
        url: &str,
        name: &str,
        etag: Option<&str>,
    ) -> Result<DownloadStatus, RunnerError> {
        let client = reqwest::blocking::Client::new();
        let mut request = client.request(reqwest::Method::GET, url);
        if let Some(etag) = etag {
            request = request.header(reqwest::header::IF_NONE_MATCH, etag);
        }
        let request = request.build().unwrap();
        log::debug!("Download {url}");
        let mut response = client.execute(request).unwrap();
        log::debug!("Download complete {url} {}", response.status());
        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .map(|h| h.to_str().unwrap().to_owned());
        let area = self.create_empty_area()?;
        let path = FilePath::new(name)?;
        let entry = FSEntry::new(area, path);
        let output_path = self.resolve_fs_entry(&entry);
        let mut out = std::fs::File::create_new(output_path).unwrap();
        std::io::copy(&mut response, &mut out).unwrap();
        // Safety: We just created the file and checked for errors so it is present
        let output = unsafe { File::new(entry) };
        Ok(DownloadStatus {
            ok: response.status().is_success(),
            status_code: Some(response.status().as_u16()),
            file: Some(output),
            etag,
        })
    }

    fn sha256(&self, file: &File) -> Result<String, RunnerError> {
        let resolved_path = self.resolve_fs_entry(file.inner());
        let mut file = std::fs::File::open(resolved_path).map_err(RunnerError::AreaIOError)?;
        let mut hasher = sha2::Sha256::new();
        std::io::copy(&mut file, &mut hasher).map_err(RunnerError::AreaIOError)?;
        let hash_result = hasher.finalize();
        Ok(base16::encode_lower(&hash_result))
    }

    fn sha512(&self, file: &File) -> Result<String, RunnerError> {
        let resolved_path = self.resolve_fs_entry(file.inner());
        let mut file = std::fs::File::open(resolved_path).map_err(RunnerError::AreaIOError)?;
        let mut hasher = sha2::Sha512::new();
        std::io::copy(&mut file, &mut hasher).map_err(RunnerError::AreaIOError)?;
        let hash_result = hasher.finalize();
        Ok(base16::encode_lower(&hash_result))
    }

    fn create_area(&self, dirs: &[&Dir]) -> Result<FileArea, RunnerError> {
        self.create_overlay_area(dirs.iter().copied())
    }

    fn read_file(&self, file: &File) -> Result<String, std::io::Error> {
        let resolved_path = self.resolve_fs_entry(file.inner());
        let contents = std::fs::read_to_string(resolved_path)?;
        Ok(contents)
    }

    fn create_file(&self, contents: &str, name: &str) -> Result<File, RunnerError> {
        let area = self.create_empty_area()?;
        let path = FilePath::new(name)?;
        let entry = FSEntry::new(area, path);
        let resolved_path = self.resolve_fs_entry(&entry);
        std::fs::write(resolved_path, contents).map_err(RunnerError::AreaIOError)?;
        // Safety: We just created the file
        let file = unsafe { File::new(entry) };
        Ok(file)
    }

    fn file_metadata(&self, file: &File) -> Result<FileMetadata, RunnerError> {
        let metadata = std::fs::metadata(self.resolve_fs_entry(file.inner()))
            .map_err(RunnerError::AreaIOError)?;
        Ok(FileMetadata {
            size: metadata.len(),
        })
    }

    #[expect(clippy::unwrap_used)]
    fn glob(&self, dir: &Dir, _pattern: &str) -> Result<Vec<File>, RunnerError> {
        let base_path = self.resolve_fs_entry(dir.inner());
        // TODO: Implement proper globbing
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
                let p = dir.path().join(p).unwrap();
                // Safety: We know this file exists, we just checked
                let file = unsafe { File::new(FSEntry::new(dir.area().clone(), p)) };
                out.push(file);
            }
        }
        Ok(out)
    }
}

impl MonitoringTrait for DriverImpl<'_> {
    fn enter_call(&self, s: &str) {
        if let Some(ph) = &self.enter_handler {
            ph(s);
        }
    }

    fn exit_call(&self, s: &str) {
        if let Some(ph) = &self.exit_handler {
            ph(s);
        }
    }

    fn enter_internal_call(&self, f: &InternalFunction) {
        if let Some(ph) = &self.enter_handler {
            ph(&format!("internal.{f:?}"));
        }
    }

    fn exit_internal_call(&self, f: &InternalFunction) {
        if let Some(ph) = &self.exit_handler {
            ph(&format!("internal.{f:?}"));
        }
    }
}

#[cfg(target_family = "unix")]
const PATH_SEPARATOR: char = ':';
#[cfg(target_family = "windows")]
const PATH_SEPARATOR: char = ';';

fn find_bin_in_dir(dir: &Path, name: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.find_map(|e| {
        let p = e.ok()?;
        if p.file_name().to_str()? == name {
            Some(p.path())
        } else {
            None
        }
    })
}
