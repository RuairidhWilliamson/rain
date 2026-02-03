#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Dir {
    Generated(super::generated::dir::GeneratedDir),
    Local(super::local::dir::LocalDir),
}

impl std::fmt::Display for Dir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Dir::Generated(generated_file) => generated_file.fmt(f),
            Dir::Local(local_file) => local_file.fmt(f),
        }
    }
}
