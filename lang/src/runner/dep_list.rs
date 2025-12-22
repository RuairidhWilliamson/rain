use crate::{afs::area::FileArea, runner::dep::Dep};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DepList {
    inner: Vec<Dep>,
}

impl DepList {
    pub fn new() -> Self {
        Self { inner: Vec::new() }
    }

    pub fn push(&mut self, dep: Dep) {
        self.inner.push(dep);
    }

    pub fn add_dep_file_area(&mut self, area: &FileArea) {
        match area {
            FileArea::Local(_) => self.inner.push(Dep::LocalArea),
            FileArea::Generated(_) => (),
        }
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
}

impl IntoIterator for DepList {
    type Item = Dep;
    type IntoIter = std::vec::IntoIter<Dep>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}
