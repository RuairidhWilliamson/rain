use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hasher},
};

use crate::ir::DeclarationId;

use super::value::{RainFunction, RainHash, RainInternalFunction, RainValue};

#[derive(Default)]
pub struct Cache {
    storage: HashMap<CacheKey, RainValue>,
}

impl Cache {
    pub fn function_key<'a>(
        &self,
        function: impl Into<FunctionDefinition>,
        args: impl Iterator<Item = &'a RainValue>,
    ) -> CacheKey {
        let mut hasher = DefaultHasher::new();
        for a in args {
            RainHash::hash(a, &mut hasher);
        }
        let args_hash = hasher.finish();
        CacheKey {
            definition: function.into(),
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
    definition: FunctionDefinition,
    args_hash: u64,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum FunctionDefinition {
    DeclarationId(DeclarationId),
    Internal(RainInternalFunction),
}

impl From<&RainFunction> for FunctionDefinition {
    fn from(f: &RainFunction) -> Self {
        Self::DeclarationId(f.id)
    }
}

impl From<RainInternalFunction> for FunctionDefinition {
    fn from(f: RainInternalFunction) -> Self {
        Self::Internal(f)
    }
}
