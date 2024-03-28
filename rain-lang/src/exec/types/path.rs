use std::path::PathBuf;

#[derive(Debug)]
pub struct Path {
    pub path: PathBuf,
    pub current_directory: PathBuf,
}

impl Path {
    pub fn absolute(&self) -> PathBuf {
        self.current_directory.join(&self.path)
    }
}
