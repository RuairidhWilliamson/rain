use std::path::PathBuf;

use super::corelib::CoreHandler;

#[derive(Debug, Default)]
pub struct GlobalExecutorBuilder {
    pub current_directory: PathBuf,
    pub core_handler: Option<Box<dyn CoreHandler>>,
    pub std_lib: Option<super::types::record::Record>,
    pub options: super::ExecuteOptions,
}

impl GlobalExecutorBuilder {
    pub fn build(self) -> GlobalExecutor {
        let current_directory = self.current_directory;
        let core_handler = self
            .core_handler
            .unwrap_or_else(|| Box::new(super::corelib::DefaultCoreHandler));
        let options = self.options;
        let mut global_record = super::types::record::Record::new([(
            String::from("core"),
            super::types::RainValue::Record(super::corelib::core_lib()),
        )]);
        if let Some(std_lib) = self.std_lib {
            global_record.insert(
                String::from("std"),
                super::types::RainValue::Record(std_lib),
            );
        }

        GlobalExecutor {
            current_directory,
            core_handler,
            global_record,
            options,
        }
    }
}

#[derive(Debug)]
pub struct GlobalExecutor {
    pub core_handler: Box<dyn CoreHandler>,
    pub current_directory: PathBuf,
    pub global_record: super::types::record::Record,
    pub options: super::ExecuteOptions,
}

#[derive(Debug)]
pub struct Executor<'a> {
    pub global_executor: &'a mut GlobalExecutor,
    pub local_record: super::types::record::Record,
}

impl<'a> Executor<'a> {
    pub fn new(global_executor: &'a mut GlobalExecutor) -> Self {
        Self {
            global_executor,
            local_record: super::types::record::Record::default(),
        }
    }
    pub fn global_executor(&mut self) -> &mut GlobalExecutor {
        self.global_executor
    }
}
