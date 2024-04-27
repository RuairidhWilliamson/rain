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

    pub fn insert_set(&mut self, set: &Self) {
        for l in &set.0 {
            self.0.push(l.clone())
        }
    }
}
