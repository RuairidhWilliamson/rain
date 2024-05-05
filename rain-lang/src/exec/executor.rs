use ordered_hash_map::OrderedHashMap;

use crate::{cache::MemCache, leaf::LeafSet, path::Workspace};

use super::{
    corelib::{core_lib, CoreHandler},
    script::ScriptExecutor,
    types::{record::Record, RainValue},
    ExecCF,
};

#[derive(Debug, Default)]
pub struct ExecutorBuilder {
    pub cache: Option<Box<dyn crate::cache::Cache>>,
    pub core_handler: Option<Box<dyn CoreHandler>>,
    pub stdlib: Option<Record>,
    pub options: super::ExecuteOptions,
}

impl ExecutorBuilder {
    pub fn build(self, root_workspace: Workspace) -> BaseExecutor {
        let core_handler = self
            .core_handler
            .unwrap_or_else(|| Box::new(super::corelib::DefaultCoreHandler));
        let stdlib = self.stdlib.map(RainValue::Record);
        let options = self.options;
        let cache = self.cache.unwrap_or_else(|| Box::<MemCache>::default());

        BaseExecutor {
            root_workspace,
            cache,
            corelib: core_lib().into(),
            core_handler,
            stdlib,
            options,
        }
    }
}

/// Base executor is held for the lifetime of a run
#[derive(Debug)]
pub struct BaseExecutor {
    pub root_workspace: Workspace,
    pub corelib: RainValue,
    pub cache: Box<dyn crate::cache::Cache>,
    pub core_handler: Box<dyn CoreHandler>,
    pub stdlib: Option<RainValue>,
    pub options: super::ExecuteOptions,
}

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
