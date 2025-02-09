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
    driver::{DownloadStatus, DriverTrait, RunStatus},
    runner::{error::RunnerError, internal::InternalFunction},
};

use crate::config::Config;

pub type PrintHandler<'a> = Box<dyn Fn(&str) + 'a>;

pub struct DriverImpl<'a> {
    pub config: Config,
    pub prints: Mutex<Vec<String>>,
    pub enter_handler: Option<PrintHandler<'a>>,
    pub exit_handler: Option<PrintHandler<'a>>,
    pub print_handler: Option<PrintHandler<'a>>,
}

impl DriverImpl<'_> {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            prints: Mutex::default(),
            enter_handler: None,
            exit_handler: None,
            print_handler: None,
        }
    }

    fn create_area(&self) -> Result<FileArea, RunnerError> {
        let area = FileArea::Generated(GeneratedFileArea::new());
        let output_dir = File::new(area.clone(), "/");
        let output_dir_path = self.resolve_file(&output_dir);
        std::fs::create_dir_all(&output_dir_path).map_err(RunnerError::AreaIOError)?;
        Ok(area)
    }

    fn create_overlay_area(&self, overlay_area: &FileArea) -> Result<FileArea, RunnerError> {
        let area = FileArea::Generated(GeneratedFileArea::new());
        let output_dir = File::new(area.clone(), "/");
        let output_dir_path = self.resolve_file(&output_dir);
        let input_dir = File::new(overlay_area.clone(), "/");
        let input_dir_path = self.resolve_file(&input_dir);
        dircpy::copy_dir(input_dir_path, &output_dir_path).map_err(RunnerError::AreaIOError)?;
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

    fn extract(&self, file: &File) -> Result<FileArea, RunnerError> {
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

    #[expect(clippy::unwrap_used)]
    fn run(
        &self,
        overlay_area: Option<&FileArea>,
        bin: &File,
        args: Vec<String>,
    ) -> Result<RunStatus, RunnerError> {
        let output_area = if let Some(overlay_area) = overlay_area {
            self.create_overlay_area(overlay_area)?
        } else {
            self.create_area()?
        };
        let output_dir = File::new(output_area.clone(), "/");
        let output_dir_path = self.resolve_file(&output_dir);
        let mut cmd = std::process::Command::new(self.resolve_file(bin));
        cmd.current_dir(output_dir_path);
        cmd.args(args);
        // TODO: It would be nice to remove env vars but for the moment this causes too many problems
        // cmd.env_clear();
        log::debug!("Running {cmd:?}");
        let exit = cmd.status().unwrap();
        let success = exit.success();
        let exit_code = exit.code();
        Ok(RunStatus {
            success,
            exit_code,
            area: output_area,
        })
    }

    #[expect(clippy::unwrap_used)]
    fn download(&self, url: &str) -> Result<DownloadStatus, RunnerError> {
        let client = reqwest::blocking::Client::new();
        let request = client
            .request(reqwest::Method::GET, url)
            // TODO: Download cache
            // TODO: Download lock file
            // TODO: ETAG support
            // .header(
            //     reqwest::header::IF_NONE_MATCH,
            //     "\"3b22f9fe438383527860677d34196a03d388c34822b85064d0e0f2a1683c91dc\"",
            // )
            .build()
            .unwrap();
        log::debug!("Sending request {request:?}");
        let mut response = client.execute(request).unwrap();
        log::debug!("Received response {response:?}");
        let area = self.create_area()?;
        let output = File::new(area, "/download");
        let output_path = self.resolve_file(&output);
        let mut out = std::fs::File::create_new(output_path).unwrap();
        std::io::copy(&mut response, &mut out).unwrap();
        Ok(DownloadStatus {
            ok: response.status().is_success(),
            status_code: Some(response.status().as_u16()),
            file: Some(output),
        })
    }

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
