use std::collections::HashMap;

use super::{DynValue, Value};

#[derive(Debug, Default)]
pub struct Record(HashMap<String, DynValue>);

impl Value for Record {
    fn get_type(&self) -> super::Type {
        super::Type::Record
    }

    fn as_record(&self) -> Result<&Record, super::Type> {
        Ok(&self)
    }
}

impl std::fmt::Display for Record {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.keys().map(|k| f.write_str(k)).collect()
    }
}

impl Record {
    pub fn insert(&mut self, k: String, v: DynValue) {
        self.0.insert(k, v);
    }

    pub fn get(&self, k: &str) -> Option<&DynValue> {
        self.0.get(k)
    }
}
