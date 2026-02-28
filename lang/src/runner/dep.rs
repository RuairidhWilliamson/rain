use crate::{
    afs::local::entry::LocalFSEntry, driver::FSTrait, hash::FileHash, runner::LocalFileHashCache,
};

#[derive(
    Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Dep {
    /// Depends on a local file, if the file is not present with the same hash the cache entry is not valid
    LocalFile(LocalFSEntry, FileHash),
    /// Marks any calls that depend on this to depend on the escaped environment
    Escape,
    /// Marks any calls that depend on this to depend on the secret
    // TODO: Specify the secret name
    Secret,
    /// Marks the call as depending on the calling module
    CallingModule,
    /// This prints so should not be cached
    Print,
    /// This depends on an environment variable
    EnvVar,
    /// This depends on a config
    Config,
    /// This increments a counter so should not be cached but should not propogate to callers
    Counter,
    /// Marks any calls that depend on this to depend on a local directory
    // TODO: Specify the local area/dir
    LocalDir,
    /// Marks any calls that depend on this to be uncacheable
    Uncacheable,
}

impl Dep {
    pub fn is_propogated_in_closure(&self) -> bool {
        !matches!(self, Self::CallingModule | Self::Counter)
    }

    pub fn is_intra_run_stable(&self) -> bool {
        match self {
            Self::Uncacheable | Self::CallingModule | Self::Print | Self::Counter => false,
            Self::LocalDir
            | Self::Escape
            | Self::Secret
            | Self::EnvVar
            | Self::Config
            | Self::LocalFile(..) => true,
        }
    }

    pub fn is_inter_run_stable(&self) -> bool {
        matches!(self, Self::LocalFile(..))
    }

    pub fn is_valid<FS: FSTrait>(&self, fs: &FS, lfhc: &mut LocalFileHashCache) -> bool {
        match self {
            Self::LocalFile(fsentry, hash) => match lfhc.hash(fsentry.clone(), fs) {
                Ok(filehash) => filehash == hash,
                Err(_) => false,
            },
            _ => true,
        }
    }
}

impl std::fmt::Display for Dep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Dep::Uncacheable => f.write_str("Uncacheable"),
            Dep::Escape => f.write_str("Escape"),
            Dep::Secret => f.write_str("Secret"),
            Dep::CallingModule => f.write_str("CallingModule"),
            Dep::Print => f.write_str("Print"),
            Dep::EnvVar => f.write_str("EnvVar"),
            Dep::Config => f.write_str("Config"),
            Dep::Counter => f.write_str("Counter"),
            Dep::LocalDir => f.write_str("LocalDir"),
            Dep::LocalFile(local_fsentry, file_hash) => {
                f.write_fmt(format_args!("LocalFile({local_fsentry}, {file_hash})"))
            }
        }
    }
}
