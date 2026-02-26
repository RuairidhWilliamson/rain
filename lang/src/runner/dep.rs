use crate::{
    afs::local::{entry::LocalFSEntry, file::LocalFile},
    driver::FSTrait,
    hash::FileHash,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Dep {
    /// Marks any calls that depend on this to be uncacheable
    Uncacheable,
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
    /// Depends on a local file, if the file is not present with the same hash the cache entry is not valid
    LocalFile(LocalFSEntry, FileHash),
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

    pub fn is_valid<FS: FSTrait>(&self, fs: &FS) -> bool {
        match self {
            Self::LocalFile(fsentry, hash) => {
                let file = LocalFile::new_checked(fs, fsentry.clone());
                match file {
                    Ok(file) => file.file_hash() == hash,
                    Err(_) => false,
                }
            }
            _ => true,
        }
    }
}
