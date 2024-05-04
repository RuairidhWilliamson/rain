use std::{ops::Deref, rc::Rc};

use ordered_hash_map::OrderedHashMap;

use super::RainValue;

#[derive(Debug, Clone, Default)]
pub struct Record(Rc<OrderedHashMap<String, RainValue>>);

impl std::fmt::Display for Record {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("{ ")?;
        self.0
            .deref()
            .iter()
            .try_for_each(|(k, v)| f.write_fmt(format_args!("{k}: {v}, ")))?;
        f.write_str("}")
    }
}

impl Record {
    pub fn new(kv: impl IntoIterator<Item = (String, RainValue)>) -> Self {
        let m = kv.into_iter().collect();
        Self(Rc::new(m))
    }

    pub fn get(&self, k: &str) -> Option<RainValue> {
        self.0.get(k).cloned()
    }
}

impl IntoIterator for Record {
    type Item = (String, RainValue);

    type IntoIter = ordered_hash_map::ordered_map::IntoIter<String, RainValue>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.deref().clone().into_iter()
    }
}

impl From<OrderedHashMap<String, RainValue>> for Record {
    fn from(map: OrderedHashMap<String, RainValue>) -> Self {
        Self(Rc::new(map))
    }
}
