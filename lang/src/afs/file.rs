#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum File {
    Generated(super::generated::file::GeneratedFile),
    Local(super::local::file::LocalFile),
}

impl std::fmt::Display for File {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            File::Generated(generated_file) => generated_file.fmt(f),
            File::Local(local_file) => local_file.fmt(f),
        }
    }
}
