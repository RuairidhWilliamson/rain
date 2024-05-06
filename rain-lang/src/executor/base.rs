use crate::{
    exec::{corelib::CoreHandler, types::RainValue, ExecuteOptions},
    path::Workspace,
};

/// Base executor is held for the lifetime of a run
#[derive(Debug)]
pub struct BaseExecutor {
    pub root_workspace: Workspace,
    pub corelib: RainValue,
    pub cache: Box<dyn crate::cache::Cache>,
    pub core_handler: Box<dyn CoreHandler>,
    pub stdlib: Option<RainValue>,
    pub options: ExecuteOptions,
}
