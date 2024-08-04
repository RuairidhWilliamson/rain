use std::{collections::HashMap, path::Path};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub dependencies: HashMap<String, Dependency>,
}

impl Manifest {
    pub fn load(path: &Path) -> Self {
        let manifest_contents = std::fs::read_to_string(path).unwrap();
        toml::de::from_str(&manifest_contents).unwrap()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub git: Option<String>,
    pub path: Option<String>,
}
