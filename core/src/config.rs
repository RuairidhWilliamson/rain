use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    pub fn server_socket_path(&self) -> PathBuf {
        self.base_run_dir.join("server.socket")
    }

    pub fn server_stderr_path(&self) -> PathBuf {
        self.base_data_dir.join("server.stderr")
    }

    pub fn server_panic_path(&self, id: uuid::Uuid) -> PathBuf {
        self.base_data_dir.join(format!("server-panic-{id}.stderr"))
    }
}
