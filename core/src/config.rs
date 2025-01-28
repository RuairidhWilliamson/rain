use std::path::PathBuf;

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
