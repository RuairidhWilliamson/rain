use std::collections::HashMap;

use crate::{
    cache::MemCache,
    exec::{
        corelib::{core_lib, CoreHandler, DefaultCoreHandler},
        types::{record::Record, RainValue},
        ExecuteOptions,
    },
    path::Workspace,
};

use super::base::BaseExecutor;

#[derive(Debug, Default)]
pub struct ExecutorBuilder {
    pub dependencies: HashMap<String, crate::manifest::Dependency>,
    pub cache: Option<Box<dyn crate::cache::Cache>>,
    pub core_handler: Option<Box<dyn CoreHandler>>,
    pub stdlib: Option<Record>,
    pub options: ExecuteOptions,
}

impl ExecutorBuilder {
    pub fn build(self, root_workspace: Workspace) -> BaseExecutor {
        let core_handler = self
            .core_handler
            .unwrap_or_else(|| Box::new(DefaultCoreHandler));
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
