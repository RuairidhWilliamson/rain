use std::path::{Path, PathBuf};

use rain_lang::{
    afs::{
        area::{FileArea, GeneratedFileArea},
        entry::FSEntry,
    },
    driver::{FSEntryQueryResult, FSTrait},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    pub base_cache_dir: PathBuf,
    pub base_generated_dir: PathBuf,
    pub base_data_dir: PathBuf,
    pub base_run_dir: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

impl Config {
    /// # Panics
    /// Panics if can't find user's directories
    pub fn new() -> Self {
        let base_cache_dir = dirs::cache_dir()
            .expect("could not find user cache directory")
            .join("rain");
        let base_generated_dir = base_cache_dir.join("generated");
        let base_data_dir = dirs::data_local_dir()
            .expect("could not find user data directory")
            .join("rain");
        let base_run_dir =
            dirs::runtime_dir().map_or_else(|| base_data_dir.clone(), |p| p.join("rain"));
        Self {
            base_cache_dir,
            base_generated_dir,
            base_data_dir,
            base_run_dir,
        }
    }

    pub fn cache_json_path(&self) -> PathBuf {
        self.base_cache_dir.join("cache.json")
    }

    #[cfg(target_family = "unix")]
    pub fn server_socket_path(&self) -> PathBuf {
        self.base_run_dir.join("server.socket")
    }

    #[cfg(target_family = "windows")]
    pub fn server_socket_path(&self) -> PathBuf {
        PathBuf::from("\\\\.\\pipe\\rain")
    }

    pub fn server_stderr_path(&self) -> PathBuf {
        self.base_data_dir.join("server.stderr")
    }

    pub fn server_panic_path(&self, id: uuid::Uuid) -> PathBuf {
        self.base_data_dir.join(format!("server-panic-{id}.stderr"))
    }

    pub fn clean_directories(&self) -> Vec<&Path> {
        let dirs: &[&Path] = &[
            &self.base_cache_dir,
            &self.base_generated_dir,
            &self.base_data_dir,
            &self.base_run_dir,
        ];
        unique_directories(dirs)
    }
}

impl FSTrait for Config {
    fn resolve_fs_entry(&self, entry: &FSEntry) -> PathBuf {
        let abs_path = entry.path.path();
        let Some(rel_path) = abs_path.strip_prefix('/') else {
            unreachable!("file path must start with /");
        };
        match &entry.area {
            FileArea::Local(p) => p.join(rel_path),
            FileArea::Generated(GeneratedFileArea { id }) => {
                self.base_generated_dir.join(id.to_string()).join(rel_path)
            }
        }
    }

    fn query_fs(&self, entry: &FSEntry) -> Result<FSEntryQueryResult, std::io::Error> {
        match std::fs::metadata(self.resolve_fs_entry(entry)) {
            Ok(m) if m.is_symlink() => Ok(FSEntryQueryResult::Symlink),
            Ok(m) if m.is_file() => Ok(FSEntryQueryResult::File),
            Ok(m) if m.is_dir() => Ok(FSEntryQueryResult::Directory),
            Ok(_) => unreachable!("file must be one of file, dir or symlink"),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Ok(FSEntryQueryResult::NotExist)
            }
            Err(err) => Err(err),
        }
    }
}

fn unique_directories<'a>(dirs: &[&'a Path]) -> Vec<&'a Path> {
    let mut dirs: Vec<&Path> = dirs
        .iter()
        .filter(|&d1| !dirs.iter().any(|d2| d1 != d2 && d1.starts_with(d2)))
        .copied()
        .collect();
    dirs.sort_unstable();
    let mut i = 1;
    while i < dirs.len() {
        if dirs[i] == dirs[i - 1] {
            dirs.remove(i);
        } else {
            i += 1;
        }
    }
    dirs
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::unique_directories;

    #[test]
    fn test_directories_unique() {
        assert_eq!(unique_directories(&[]), Vec::<&Path>::default());
        assert_eq!(
            unique_directories(&[Path::new("/foo"), Path::new("/foo/bar"), Path::new("/foo")]),
            vec![Path::new("/foo")]
        );
        assert_eq!(
            unique_directories(&[Path::new("/foo"), Path::new("/foo"), Path::new("/foo")]),
            vec![Path::new("/foo")]
        );
    }
}
