#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalSpan {
    start: usize,
    end: usize,
}

impl LocalSpan {
    pub fn byte(index: usize) -> Self {
        Self {
            start: index,
            end: index + 1,
        }
    }

    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}
