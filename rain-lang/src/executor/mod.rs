use ordered_hash_map::OrderedHashMap;

use crate::{
    exec::{corelib::CoreHandler, types::RainValue},
    leaf::LeafSet,
};

use self::{base::BaseExecutor, script::ScriptExecutor};

use super::ExecCF;

pub mod base;
pub mod builder;
pub mod script;

/// Executor is held for the lifetime of a function call, a new Executor is created for each function call, except external function calls
#[derive(Debug)]
#[non_exhaustive]
pub struct Executor<'a> {
    pub base_executor: &'a mut BaseExecutor,
    pub script_executor: &'a ScriptExecutor,
    pub local_record: OrderedHashMap<String, RainValue>,
    pub call_depth: usize,
    pub leaves: LeafSet,
}

impl BaseExecutor {
    pub fn resolve(&self, name: &str) -> Option<&RainValue> {
        match name {
            "core" => Some(&self.corelib),
            "std" => self.stdlib.as_ref(),
            _ => None,
        }
    }
}

impl<'a> Executor<'a> {
    pub fn new(base_executor: &'a mut BaseExecutor, script_executor: &'a ScriptExecutor) -> Self {
        Self {
            base_executor,
            script_executor,
            local_record: OrderedHashMap::default(),
            call_depth: 0,
            leaves: LeafSet::default(),
        }
    }

    pub fn core_handler(&mut self) -> &mut Box<dyn CoreHandler> {
        &mut self.base_executor.core_handler
    }

    pub fn resolve(&mut self, name: &str) -> Option<Result<RainValue, ExecCF>> {
        self.local_record
            .get(name)
            .map(|v| Ok(v.clone()))
            .or_else(|| self.script_executor.resolve(name, self.base_executor))
            .or_else(|| self.base_executor.resolve(name).map(|v| Ok(v.clone())))
    }
}
