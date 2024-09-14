#[derive(Debug)]
pub struct AppendVec<T> {
    inner: Vec<T>,
}

impl<T> Default for AppendVec<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> AppendVec<T> {
    pub const fn new() -> Self {
        Self { inner: Vec::new() }
    }

    pub fn push(&mut self, elem: T) -> usize {
        let index = self.inner.len();
        self.inner.push(elem);
        index
    }
}
