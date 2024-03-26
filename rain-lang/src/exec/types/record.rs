use std::{cell::RefCell, collections::HashMap, ops::Deref, rc::Rc};

use super::RainValue;

#[derive(Debug, Clone, Default)]
pub struct Record(Rc<RefCell<HashMap<String, RainValue>>>);

impl std::fmt::Display for Record {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.borrow().deref().keys().try_for_each(|k| {
            f.write_str(k)?;
            f.write_str(",")
        })
    }
}

impl Record {
    pub fn new(kv: impl IntoIterator<Item = (String, RainValue)>) -> Self {
        let m = kv.into_iter().collect();
        Self(Rc::new(RefCell::new(m)))
    }

    pub fn insert(&mut self, k: String, v: RainValue) {
        self.0.borrow_mut().insert(k, v);
    }

    pub fn get(&self, k: &str) -> Option<RainValue> {
        self.0.borrow().get(k).cloned()
    }
}
