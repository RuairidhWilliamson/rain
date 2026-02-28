use termcolor::WriteColor;

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
    Download,
}

impl Dep {
    pub fn is_propogated_in_closure(&self) -> bool {
        !matches!(self, Self::CallingModule | Self::Counter)
    }

    pub fn is_intra_run_stable(&self) -> bool {
        match self {
            Self::Uncacheable
            | Self::CallingModule
            | Self::Print
            | Self::Counter
            | Self::Download => false,
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

    pub fn write_color(&self, writer: &mut impl WriteColor) -> std::io::Result<()> {
        use termcolor::{Color, ColorSpec};

        let mut color = ColorSpec::new();
        if self.is_inter_run_stable() {
            color.set_fg(Some(Color::White));
        } else if self.is_intra_run_stable() {
            color.set_fg(Some(Color::Magenta));
        } else {
            color.set_fg(Some(Color::Red));
        }
        writer.set_color(&color)?;
        match self {
            Self::Uncacheable => write!(writer, "Uncacheable")?,
            Self::Escape => write!(writer, "Escape")?,
            Self::Secret => write!(writer, "Secret")?,
            Self::CallingModule => write!(writer, "CallingModule")?,
            Self::Print => write!(writer, "Print")?,
            Self::EnvVar => write!(writer, "EnvVar")?,
            Self::Config => write!(writer, "Config")?,
            Self::Counter => write!(writer, "Counter")?,
            Self::LocalDir => write!(writer, "LocalDir")?,
            Self::Download => write!(writer, "Download")?,
            Self::LocalFile(local_fsentry, file_hash) => {
                write!(writer, "LocalFile(")?;
                local_fsentry.write_color(writer)?;
                writer.set_color(&color)?;
                write!(writer, ", {file_hash})")?;
            }
        }
        writer.reset()?;
        Ok(())
    }
}
