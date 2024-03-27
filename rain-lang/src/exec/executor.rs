use std::path::{Path, PathBuf};

use super::{
    corelib::{core_lib, CoreHandler},
    types::{record::Record, RainValue},
};

#[derive(Debug, Default)]
pub struct ExecutorBuilder {
    pub current_directory: PathBuf,
    pub core_handler: Option<Box<dyn CoreHandler>>,
    pub std_lib: Option<Record>,
    pub options: super::ExecuteOptions,
}

impl ExecutorBuilder {
    pub fn build(self) -> BaseExecutor {
        let current_directory = self.current_directory;
        let core_handler = self
            .core_handler
            .unwrap_or_else(|| Box::new(super::corelib::DefaultCoreHandler));
        let options = self.options;
        let base_record = super::types::record::Record::new([
            (String::from("core"), core_lib().into()),
            (
                String::from("std"),
                self.std_lib
                    .map(|s| s.into())
                    .unwrap_or(super::types::RainValue::Void),
            ),
        ]);

        BaseExecutor {
            current_directory,
            core_handler,
            base_record,
            options,
        }
    }
}

#[derive(Debug)]
pub struct BaseExecutor {
    pub current_directory: PathBuf,
    pub core_handler: Box<dyn CoreHandler>,
    pub base_record: super::types::record::Record,
    pub options: super::ExecuteOptions,
}

#[derive(Debug)]
pub struct ScriptExecutor {
    pub current_directory: PathBuf,
    pub global_record: super::types::record::Record,
}

#[derive(Debug)]
pub struct Executor<'a> {
    pub base_executor: &'a mut BaseExecutor,
    pub script_executor: &'a mut ScriptExecutor,
    pub local_record: super::types::record::Record,
}

impl BaseExecutor {
    pub fn resolve(&self, name: &str) -> Option<RainValue> {
        self.base_record.get(name)
    }
}

impl ScriptExecutor {
    pub fn new(base: &BaseExecutor) -> Self {
        Self {
            current_directory: base.current_directory.clone(),
            global_record: super::types::record::Record::default(),
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
