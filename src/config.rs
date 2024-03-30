use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub fn config_search_paths(workspace_root: &Path) -> impl Iterator<Item = PathBuf> {
    [
        dirs::config_dir()
            .expect("config directory")
            .join("rain.toml"),
        workspace_root.join("rain.toml"),
    ]
    .into_iter()
}

pub fn load(workspace_root: &Path) -> Config {
    config_search_paths(workspace_root).fold(Config::default(), |resolved, path| {
        // Try opening the config file but if we can't just ignore it
        let Ok(contents) = std::fs::read_to_string(path) else {
            return resolved;
        };
        let config: UnresolvedConfig = toml::de::from_str(&contents).unwrap();
        config.merge(resolved)
    })
}

#[derive(Debug, Default, Clone, Deserialize)]
struct UnresolvedConfig {
    pub cache_directory: Option<PathBuf>,
}

impl UnresolvedConfig {
    fn merge(self, resolved: Config) -> Config {
        Config {
            cache_directory: self.cache_directory.unwrap_or(resolved.cache_directory),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Config {
    pub cache_directory: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cache_directory: dirs::cache_dir().expect("cache directory").join("rain"),
        }
    }
}
