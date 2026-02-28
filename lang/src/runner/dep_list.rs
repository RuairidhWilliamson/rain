use std::collections::HashSet;

use crate::runner::dep::Dep;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DepList {
    inner: Vec<Dep>,
}

impl DepList {
    pub fn new() -> Self {
        Self { inner: Vec::new() }
    }

    pub fn len(&self) -> usize {
        self.inner.len()
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
}

impl IntoIterator for DepList {
    type Item = Dep;
    type IntoIter = std::vec::IntoIter<Dep>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}
