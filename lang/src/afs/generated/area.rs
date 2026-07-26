use std::sync::Arc;

use crate::runner::value::Value;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct GeneratedFSArea {
    pub id: uuid::Uuid,
    pub git_describe: Option<GitDescribe>,
}

impl Default for GeneratedFSArea {
    fn default() -> Self {
        Self::new()
    }
}

impl GeneratedFSArea {
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            git_describe: None,
        }
    }

    pub fn to_value(self) -> Value {
        Value::GeneratedFSArea(Arc::new(self))
    }
}

impl std::fmt::Display for GeneratedFSArea {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            id,
            git_describe: _,
        } = self;
        f.write_fmt(format_args!("{id}"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct GitDescribe {
    pub commit: String,
    pub dirty: bool,
}
