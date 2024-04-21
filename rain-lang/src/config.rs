use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};

static CONFIG: Mutex<Option<&'static Config>> = Mutex::new(None);

pub fn global_config() -> &'static Config {
    CONFIG.lock().unwrap().expect("CONFIG not set")
}

pub fn set_global_config(config: Config) {
    let mut guard = CONFIG.lock().unwrap();
    if guard.is_some() {
        panic!("CONFIG is already set");
    }
    *guard = Some(Box::leak(Box::new(config)));
}

pub fn config_search_paths(root_workspace_directory: &Path) -> impl Iterator<Item = PathBuf> {
    [
        dirs::config_dir()
            .expect("config directory")
            .join("rain.toml"),
        root_workspace_directory.join("rain.toml"),
    ]
    .into_iter()
}

pub fn load(root_workspace_directory: &Path) -> UnvalidatedConfig {
    config_search_paths(root_workspace_directory).fold(
        UnvalidatedConfig::default(),
        |resolved, path| {
            // Try opening the config file but if we can't just ignore it
            let Ok(contents) = std::fs::read_to_string(path) else {
                return resolved;
            };
            let config: UnresolvedConfig = toml::de::from_str(&contents).unwrap();
            config.merge(resolved)
        },
    )
}

#[derive(Debug, Default, Clone, Deserialize)]
struct UnresolvedConfig {
    pub cache_directory: Option<PathBuf>,
}

impl UnresolvedConfig {
    fn merge(self, resolved: UnvalidatedConfig) -> UnvalidatedConfig {
        UnvalidatedConfig(Config {
            cache_directory: self.cache_directory.unwrap_or(resolved.0.cache_directory),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ValidateError {
    #[error("io error {0}")]
    IOError(std::io::Error),
    #[error("rain_marker missing")]
    RainMarkerMissing(std::io::Error),
    #[error("rain_marker contents do not match expected")]
    RainMarkerContentsUnexpected,
}

#[derive(Debug, Clone)]
pub struct UnvalidatedConfig(Config);

impl Default for UnvalidatedConfig {
    fn default() -> Self {
        Self(Config {
            cache_directory: dirs::cache_dir().expect("cache directory").join("rain"),
        })
    }
}

impl UnvalidatedConfig {
    pub fn validate(self) -> Result<Config, ValidateError> {
        self.validate_cache_directory()?;
        Ok(self.0)
    }

    fn validate_cache_directory(&self) -> Result<(), ValidateError> {
        if !self
            .0
            .cache_directory
            .try_exists()
            .map_err(ValidateError::IOError)?
        {
            tracing::info!("Cache directory does not exist, creating it");
            // If the cache directory does not exist we should create it
            std::fs::create_dir_all(&self.0.cache_directory).map_err(ValidateError::IOError)?;
        }
        let marker_path = self.0.cache_directory.join("rain_marker");
        const MARKER_CONTENTS: &str = "rain directory, use rain to manipulate the files here";
        if self
            .0
            .cache_directory
            .read_dir()
            .map_err(ValidateError::IOError)?
            .count()
            == 0
        {
            // If the cache directory is empty this is a valid cache directory but we should create the rain marker
            tracing::info!("Cache directory is empty, creating rain_marker");
            std::fs::write(&marker_path, MARKER_CONTENTS).map_err(ValidateError::IOError)?;
        }
        let marker =
            std::fs::read_to_string(&marker_path).map_err(ValidateError::RainMarkerMissing)?;
        if marker == MARKER_CONTENTS {
            tracing::debug!("validated marker contents");
            Ok(())
        } else {
            // If the rain_marker file's contents do not match what we expect this is an error
            tracing::error!(
                expected = MARKER_CONTENTS,
                actual = marker,
                "marker contents do not match expected",
            );
            Err(ValidateError::RainMarkerContentsUnexpected)
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Config {
    pub cache_directory: PathBuf,
}

impl Config {
    pub fn exec_directory(&self) -> PathBuf {
        self.cache_directory.join("exec")
    }

    pub fn generated_directory(&self) -> PathBuf {
        self.cache_directory.join("generated")
    }

    pub fn out_directory(&self) -> PathBuf {
        self.cache_directory.join("out")
    }
}
