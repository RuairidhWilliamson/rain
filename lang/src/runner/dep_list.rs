use std::collections::HashSet;

use crate::runner::{dep::Dep, value::Value};

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DepList {
    inner: Vec<Dep>,
}

impl std::fmt::Debug for DepList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("DepList").field(&self.inner).finish()
    }
}

impl DepList {
    pub fn new() -> Self {
        Self { inner: Vec::new() }
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn push(&mut self, dep: Dep) {
        self.inner.push(dep);
    }

    pub fn extend(&mut self, deps: impl Iterator<Item = Dep>) {
        self.inner.extend(deps);
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }

    pub fn iter(&self) -> impl Iterator<Item = &Dep> {
        self.inner.iter()
    }

    pub fn merge(&mut self, other: Self) {
        self.inner.extend(other.inner);
    }

    pub fn unique(&self) -> HashSet<Dep> {
        self.iter().cloned().collect()
    }

    pub fn sort_and_unique(&mut self) {
        self.inner.sort();
        self.inner.dedup();
    }

    pub fn add_based_on_value(&mut self, value: &Value) {
        match value {
            Value::Unit
            | Value::Boolean(_)
            | Value::Integer(_)
            | Value::String(_)
            | Value::Module(_)
            | Value::GeneratedFSArea(_)
            | Value::GeneratedFile(_)
            | Value::GeneratedDir(_)
            | Value::Type(_)
            | Value::Internal
            | Value::Closure(_)
            | Value::InternalFunction(_) => {}
            Value::LocalFSArea(_) | Value::LocalDir(_) => self.push(Dep::LocalDir),
            Value::LocalFile(local_file) => self.push(Dep::LocalFile(
                local_file.fsinner().clone(),
                local_file.file_hash().clone(),
            )),
            Value::EscapeFile(_) => self.push(Dep::Escape),
            Value::List(list) => list.0.iter().for_each(|v| self.add_based_on_value(v)),
            Value::Record(record) => record
                .0
                .iter()
                .for_each(|(_, v)| self.add_based_on_value(v)),
        }
    }
}

impl IntoIterator for DepList {
    type Item = Dep;
    type IntoIter = std::vec::IntoIter<Dep>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}
