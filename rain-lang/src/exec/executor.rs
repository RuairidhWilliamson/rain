use crate::{leaf::LeafSet, path::Workspace, source::Source};

use super::{
    corelib::{core_lib, CoreHandler},
    types::{record::Record, RainValue},
};

#[derive(Debug, Default)]
pub struct ExecutorBuilder {
    pub corelib_handler: Option<Box<dyn CoreHandler>>,
    pub stdlib: Option<Record>,
    pub options: super::ExecuteOptions,
}

impl ExecutorBuilder {
    pub fn build(self, root_workspace: Workspace) -> BaseExecutor {
        let corelib_handler = self
            .corelib_handler
            .unwrap_or_else(|| Box::new(super::corelib::DefaultCoreHandler));
        let stdlib = self.stdlib.map(RainValue::Record);
        let options = self.options;

        BaseExecutor {
            root_workspace,
            corelib: core_lib().into(),
            core_handler: corelib_handler,
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
    pub core_handler: Box<dyn CoreHandler>,
    pub stdlib: Option<RainValue>,
    pub options: super::ExecuteOptions,
}

/// ScriptExecutor is held for the lifetime of a script, a new ScriptExecutor is created for each core.import
#[derive(Debug)]
pub struct ScriptExecutor {
    pub source: Source,
    pub global_record: super::types::record::Record,
}

/// Executor is held for the lifetime of a function call, a new Executor is created for each function call, except external function calls
#[derive(Debug)]
#[non_exhaustive]
pub struct Executor<'a> {
    pub base_executor: &'a mut BaseExecutor,
    pub script_executor: &'a mut ScriptExecutor,
    pub local_record: super::types::record::Record,
    pub call_depth: usize,
    pub leaves: LeafSet,
}

impl BaseExecutor {
    pub fn resolve(&self, name: &str) -> Option<RainValue> {
        match name {
            "core" => Some(self.corelib.clone()),
            "std" => self.stdlib.as_ref().cloned(),
            _ => None,
        }
    }
}

impl ScriptExecutor {
    pub fn new(source: Source) -> Self {
        Self {
            global_record: Record::default(),
            source,
        }
    }

    pub fn resolve(&self, name: &str) -> Option<RainValue> {
        self.global_record.get(name)
    }
}

impl<'a> Executor<'a> {
    pub fn new(
        base_executor: &'a mut BaseExecutor,
        script_executor: &'a mut ScriptExecutor,
    ) -> Self {
        Self {
            base_executor,
            script_executor,
            local_record: super::types::record::Record::default(),
            call_depth: 0,
            leaves: LeafSet::default(),
        }
    }

    pub fn core_handler(&mut self) -> &mut Box<dyn CoreHandler> {
        &mut self.base_executor.core_handler
    }

    pub fn global_record(&mut self) -> &mut super::types::record::Record {
        &mut self.script_executor.global_record
    }

    pub fn resolve(&self, name: &str) -> Option<RainValue> {
        self.local_record
            .get(name)
            .or_else(|| self.script_executor.resolve(name))
            .or_else(|| self.base_executor.resolve(name))
    }
}
