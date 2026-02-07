use std::sync::Arc;

use crate::{afs::area::FileArea, runner::value::Value};

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct GeneratedFileArea {
    pub id: uuid::Uuid,
}

impl Default for GeneratedFileArea {
    fn default() -> Self {
        Self::new()
    }
}

impl GeneratedFileArea {
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
        }
    }

    pub fn to_value(self) -> Value {
        Value::FileArea(Arc::new(FileArea::Generated(self)))
    }
}

impl std::fmt::Display for GeneratedFileArea {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { id } = self;
        f.write_fmt(format_args!("{id}"))
    }
}
