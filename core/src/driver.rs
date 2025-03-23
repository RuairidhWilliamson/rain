use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use poison_panic::MutexExt as _;
use rain_lang::{
    afs::{
        area::{FileArea, GeneratedFileArea},
        file::File,
    },
    driver::{DownloadStatus, DriverTrait, MonitoringTrait, RunOptions, RunStatus},
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

    fn create_area(&self) -> Result<FileArea, RunnerError> {
        let area = FileArea::Generated(GeneratedFileArea::new());
        let output_dir = File::new(area.clone(), "/");
        let output_dir_path = self.resolve_file(&output_dir);
        std::fs::create_dir_all(&output_dir_path).map_err(RunnerError::AreaIOError)?;
        Ok(area)
    }

    #[expect(clippy::unwrap_used)]
    fn create_overlay_area(&self, overlay_dirs: &[&File]) -> Result<FileArea, RunnerError> {
        let area = FileArea::Generated(GeneratedFileArea::new());
        let output_dir = File::new(area.clone(), "/");
        let output_dir_path = self.resolve_file(&output_dir);
        std::fs::create_dir_all(&output_dir_path).map_err(RunnerError::AreaIOError)?;
        for &dir in overlay_dirs {
            let dir_path = self.resolve_file(dir);
            let dir_metadata = dir_path.metadata().unwrap();
            if dir_metadata.is_dir() {
                dircpy::copy_dir(dir_path, &output_dir_path).map_err(RunnerError::AreaIOError)?;
            } else if dir_metadata.is_file() {
                std::fs::copy(
                    &dir_path,
                    output_dir_path.join(dir_path.file_name().unwrap()),
                )
                .unwrap();
            }
        }
        Ok(area)
    }
}

impl DriverTrait for DriverImpl<'_> {
    fn resolve_file(&self, file: &File) -> PathBuf {
        let abs_path = file.path();
        let Some(rel_path) = abs_path.strip_prefix('/') else {
            unreachable!("file path must start with /");
        };
        match &file.area {
            FileArea::Local(p) => p.join(rel_path),
            FileArea::Generated(GeneratedFileArea { id }) => self
                .config
                .base_generated_dir
                .join(id.to_string())
                .join(rel_path),
            FileArea::Escape => PathBuf::from(abs_path),
        }
    }

    fn exists(&self, file: &File) -> Result<bool, std::io::Error> {
        self.resolve_file(file).try_exists()
    }

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
        let resolved_path = self.resolve_file(file);
        let area = self.create_area()?;
        let output_dir = File::new(area.clone(), "/");
        let output_dir_path = self.resolve_file(&output_dir);
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
            let mut out = std::fs::File::create_new(output_dir_path.join(name))
                .map_err(RunnerError::AreaIOError)?;
            std::io::copy(&mut zip_file, &mut out).map_err(RunnerError::AreaIOError)?;
        }
        Ok(area)
    }

    fn extract_tar_gz(&self, file: &File) -> Result<FileArea, RunnerError> {
        let resolved_path = self.resolve_file(file);
        let area = self.create_area()?;
        let output_dir = File::new(area.clone(), "/");
        let output_dir_path = self.resolve_file(&output_dir);
        let f = std::fs::File::open(resolved_path).map_err(RunnerError::AreaIOError)?;
        let raw_tar = flate2::read::GzDecoder::new(f);
        let mut archive = tar::Archive::new(raw_tar);
        archive
            .unpack(output_dir_path)
            .map_err(|err| RunnerError::ExtractError(Box::new(err)))?;
        Ok(area)
    }

    fn extract_tar_xz(&self, file: &File) -> Result<FileArea, RunnerError> {
        let resolved_path = self.resolve_file(file);
        let area = self.create_area()?;
        let output_dir = File::new(area.clone(), "/");
        let output_dir_path = self.resolve_file(&output_dir);
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
            self.create_overlay_area(&[&File::new(overlay_area.clone(), "/")])?
        } else {
            self.create_area()?
        };
        let output_dir = File::new(output_area.clone(), "/");
        let output_dir_path = self.resolve_file(&output_dir);
        let mut cmd = std::process::Command::new(self.resolve_file(bin));
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
        // TODO: Download cache
        let request = request.build().unwrap();
        log::debug!("Sending request {request:?}");
        let mut response = client.execute(request).unwrap();
        log::debug!("Received response {response:?}");
        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .map(|h| h.to_str().unwrap().to_owned());
        let area = self.create_area()?;
        let output = File::new_checked(area, name)?;
        let output_path = self.resolve_file(&output);
        let mut out = std::fs::File::create_new(output_path).unwrap();
        std::io::copy(&mut response, &mut out).unwrap();
        Ok(DownloadStatus {
            ok: response.status().is_success(),
            status_code: Some(response.status().as_u16()),
            file: Some(output),
            etag,
        })
    }

    fn sha256(&self, file: &File) -> Result<String, RunnerError> {
        let resolved_path = self.resolve_file(file);
        let mut file = std::fs::File::open(resolved_path).map_err(RunnerError::AreaIOError)?;
        let mut hasher = sha2::Sha256::new();
        std::io::copy(&mut file, &mut hasher).map_err(RunnerError::AreaIOError)?;
        let hash_result = hasher.finalize();
        Ok(base16::encode_lower(&hash_result))
    }

    fn sha512(&self, file: &File) -> Result<String, RunnerError> {
        let resolved_path = self.resolve_file(file);
        let mut file = std::fs::File::open(resolved_path).map_err(RunnerError::AreaIOError)?;
        let mut hasher = sha2::Sha512::new();
        std::io::copy(&mut file, &mut hasher).map_err(RunnerError::AreaIOError)?;
        let hash_result = hasher.finalize();
        Ok(base16::encode_lower(&hash_result))
    }

    fn merge_dirs(&self, dirs: &[&File]) -> Result<FileArea, RunnerError> {
        self.create_overlay_area(dirs)
    }

    fn read_file(&self, file: &File) -> Result<String, std::io::Error> {
        let resolved_path = self.resolve_file(file);
        let contents = std::fs::read_to_string(resolved_path)?;
        Ok(contents)
    }

    fn write_file(&self, contents: &str, name: &str) -> Result<File, RunnerError> {
        let area = self.create_area()?;
        let file = File::new_checked(area, name)?;
        let resolved_path = self.resolve_file(&file);
        std::fs::write(resolved_path, contents).map_err(RunnerError::AreaIOError)?;
        Ok(file)
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
