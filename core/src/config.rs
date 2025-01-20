use std::path::{Path, PathBuf};

use rain_lang::afs::{
    area::{FileArea, GeneratedFileArea},
    file::File,
    file_system::FileSystem,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub base_cache_dir: PathBuf,
    pub base_generated_dir: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

impl Config {
    /// # Panics
    /// Panics if can't find user's cache directory
    pub fn new() -> Self {
        let base_cache_dir = dirs::cache_dir()
            .expect("could not find user cache directory")
            .join("rain");
        let base_generated_dir = base_cache_dir.join("generated");
        Self {
            base_cache_dir,
            base_generated_dir,
        }
    }

    pub fn server_socket_path(&self) -> PathBuf {
        self.base_cache_dir.join("server.socket")
    }

    pub fn server_stderr_path(&self) -> PathBuf {
        self.base_cache_dir.join("server.stderr")
    }
}

impl FileSystem for Config {
    fn resolve_file(&self, file: &File) -> PathBuf {
        let abs_path = file.path();
        let Some(rel_path) = abs_path.strip_prefix('/') else {
            unreachable!("file path must start with /");
        };
        match &file.area {
            FileArea::Local(p) => p.join(rel_path),
            FileArea::Generated(GeneratedFileArea { id }) => {
                self.base_generated_dir.join(id.to_string()).join(rel_path)
            }
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
