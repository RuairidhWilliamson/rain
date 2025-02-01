use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use rain_lang::{
    afs::{
        area::{FileArea, GeneratedFileArea},
        file::File,
    },
    driver::{DriverTrait, RunStatus},
};

use crate::config::Config;

pub struct DriverImpl {
    pub config: Config,
    pub prints: Mutex<Vec<String>>,
}

impl DriverImpl {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            prints: Mutex::default(),
        }
    }
}

impl DriverTrait for DriverImpl {
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

    #[expect(clippy::unwrap_used)]
    fn print(&self, message: String) {
        self.prints.lock().unwrap().push(message);
    }

    fn extract(&self, file: &File) -> Result<FileArea, Box<dyn std::error::Error>> {
        let resolved_path = self.resolve_file(file);
        let gen_area = GeneratedFileArea::new();
        let area = FileArea::Generated(gen_area);
        let output_dir = File::new(area.clone(), "/")?;
        let output_dir_path = self.resolve_file(&output_dir);
        std::fs::create_dir_all(&output_dir_path)?;
        let f = std::fs::File::open(resolved_path)?;
        let mut zip = zip::read::ZipArchive::new(f)?;
        for i in 0..zip.len() {
            let mut zip_file = zip.by_index(i)?;
            let Some(name) = zip_file.enclosed_name() else {
                continue;
            };
            let mut out = std::fs::File::create_new(output_dir_path.join(name))?;
            std::io::copy(&mut zip_file, &mut out)?;
        }
        Ok(area)
    }

    #[expect(clippy::unwrap_used)]
    fn run(&self, overlay_area: Option<&FileArea>, bin: &File, args: Vec<String>) -> RunStatus {
        let output_area = FileArea::Generated(GeneratedFileArea::new());
        let output_dir = File::new(output_area.clone(), "/").unwrap();
        let output_dir_path = self.resolve_file(&output_dir);
        if let Some(overlay_area) = overlay_area {
            let input_dir = File::new(overlay_area.clone(), "/").unwrap();
            let input_dir_path = self.resolve_file(&input_dir);
            dircpy::copy_dir(input_dir_path, &output_dir_path).unwrap();
        } else {
            std::fs::create_dir_all(&output_dir_path).unwrap();
        };
        let mut cmd = std::process::Command::new(self.resolve_file(bin));
        cmd.current_dir(output_dir_path);
        cmd.args(args);
        // TODO: It would be nice to remove env vars but for the moment this causes too many problems
        // cmd.env_clear();
        log::debug!("Running {cmd:?}");
        let exit = cmd.status().unwrap();
        let success = exit.success();
        let exit_code = exit.code();
        RunStatus {
            success,
            exit_code,
            area: output_area,
        }
    }

    #[expect(clippy::unwrap_used)]
    fn download(&self, url: &str) -> File {
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
        let gen_area = GeneratedFileArea::new();
        let area = FileArea::Generated(gen_area);
        let output = File::new(area, "/download").unwrap();
        let output_path = self.resolve_file(&output);
        let output_dir_path = output_path.parent().unwrap();
        std::fs::create_dir_all(output_dir_path).unwrap();
        let mut out = std::fs::File::create_new(output_path).unwrap();
        std::io::copy(&mut response, &mut out).unwrap();
        output
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
