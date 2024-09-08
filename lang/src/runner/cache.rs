use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hasher},
};

use crate::ir::DeclarationId;

use super::value::{RainFunction, RainHash, RainValue};

#[derive(Default)]
pub struct Cache {
    storage: HashMap<CacheKey, RainValue>,
}

impl Cache {
    pub fn function_call_key(&mut self, function: &RainFunction, args: &[RainValue]) -> CacheKey {
        let mut hasher = DefaultHasher::new();
        for a in args {
            RainHash::hash(a, &mut hasher);
        }
        let args_hash = hasher.finish();
        CacheKey {
            declaration_id: function.id,
            args_hash,
        }
    }

    pub fn get(&mut self, key: &CacheKey) -> Option<&RainValue> {
        self.storage.get(key)
    }

    pub fn put(&mut self, key: CacheKey, v: RainValue) {
        self.storage.insert(key, v);
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct CacheKey {
    declaration_id: DeclarationId,
    args_hash: u64,
}
