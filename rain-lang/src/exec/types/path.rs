use std::path::PathBuf;

#[derive(Debug)]
pub struct Path {
    pub path: PathBuf,
    pub current_directory: PathBuf,
}

impl Path {
    pub fn relative_workspace(&self) -> PathBuf {
        self.current_directory.join(&self.path)
    }

    pub fn absolute(&self, executor: &mut crate::exec::executor::Executor) -> PathBuf {
        executor
            .base_executor
            .workspace_directory
            .join(self.relative_workspace())
            .canonicalize()
            .unwrap()
    }
}
