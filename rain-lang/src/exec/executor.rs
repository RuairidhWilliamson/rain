use std::path::{Path, PathBuf};

use crate::source::Source;

use super::{
    corelib::{core_lib, CoreHandler},
    types::{record::Record, RainValue},
};

#[derive(Debug, Default)]
pub struct ExecutorBuilder {
    pub workspace_directory: PathBuf,
    pub corelib_handler: Option<Box<dyn CoreHandler>>,
    pub stdlib: Option<Record>,
    pub options: super::ExecuteOptions,
}

impl ExecutorBuilder {
    pub fn build(self) -> BaseExecutor {
        let workspace_directory = self.workspace_directory;
        let corelib_handler = self
            .corelib_handler
            .unwrap_or_else(|| Box::new(super::corelib::DefaultCoreHandler));
        let stdlib = self.stdlib.map(RainValue::Record);
        let options = self.options;

        BaseExecutor {
            workspace_directory,
            corelib: core_lib().into(),
            core_handler: corelib_handler,
            stdlib,
            options,
        }
    }
}

#[derive(Debug)]
pub struct BaseExecutor {
    pub workspace_directory: PathBuf,
    pub corelib: RainValue,
    pub core_handler: Box<dyn CoreHandler>,
    pub stdlib: Option<RainValue>,
    pub options: super::ExecuteOptions,
}

#[derive(Debug)]
pub struct ScriptExecutor {
    pub current_directory: PathBuf,
    pub global_record: super::types::record::Record,
    pub source: Source,
}

#[derive(Debug)]
pub struct Executor<'a> {
    pub base_executor: &'a mut BaseExecutor,
    pub script_executor: &'a mut ScriptExecutor,
    pub local_record: super::types::record::Record,
    pub call_depth: usize,
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
    pub fn new(current_directory: &Path, source: Source) -> Self {
        Self {
            current_directory: current_directory.to_path_buf(),
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
        }
    }

    pub fn core_handler(&mut self) -> &mut Box<dyn CoreHandler> {
        &mut self.base_executor.core_handler
    }

    pub fn global_record(&mut self) -> &mut super::types::record::Record {
        &mut self.script_executor.global_record
    }

    pub fn current_directory(&self) -> &Path {
        &self.script_executor.current_directory
    }

    pub fn resolve(&self, name: &str) -> Option<RainValue> {
        self.local_record
            .get(name)
            .or_else(|| self.script_executor.resolve(name))
            .or_else(|| self.base_executor.resolve(name))
    }
}
