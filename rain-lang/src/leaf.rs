#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Leaf {
    File(crate::path::RainPath),
}

#[derive(Debug, Default, Clone)]
pub struct LeafSet(Vec<Leaf>);

impl LeafSet {
    pub fn insert(&mut self, leaf: Leaf) {
        if !self.0.contains(&leaf) {
            self.0.push(leaf);
        }
    }
}
