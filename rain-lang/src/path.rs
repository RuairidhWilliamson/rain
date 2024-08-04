use std::path::PathBuf;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct RainPath {
    /// Scope of the rain path
    scope: RainPathScope,
    /// Path relative to scope
    path: PathBuf,
}

impl std::fmt::Display for RainPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.scope {
            RainPathScope::Escaped => f.write_str("@escape"),
            RainPathScope::Workspace(Workspace::Local(p)) => p.display().fmt(f),
            RainPathScope::Workspace(Workspace::Remote(_)) => todo!(),
            RainPathScope::Generated(id) => f.write_fmt(format_args!("@generated/{id}")),
        }?;
        f.write_str("/@/")?;
        self.path.display().fmt(f)
    }
}

impl RainPath {
    pub fn escaped(path: PathBuf) -> Self {
        Self {
            scope: RainPathScope::Escaped,
            path,
        }
    }

    pub fn generated(id: uuid::Uuid, path: PathBuf) -> Self {
        Self {
            scope: RainPathScope::Generated(id),
            path,
        }
    }

    pub fn parent(&self) -> Option<Self> {
        Some(Self {
            scope: self.scope.clone(),
            path: self.path.parent()?.to_path_buf(),
        })
    }

    pub fn join(&self, path: impl AsRef<std::path::Path>) -> Self {
        let mut path = self.path.join(path);
        if path.starts_with("./") {
            path = path.strip_prefix("./").unwrap().to_path_buf();
        }
        Self {
            scope: self.scope.clone(),
            path,
        }
    }

    pub fn scope(&self) -> &RainPathScope {
        &self.scope
    }

    pub fn workspace_relative_directory(&self) -> &std::path::Path {
        &self.path
    }

    pub fn resolve(&self) -> PathBuf {
        match &self.scope {
            RainPathScope::Escaped => self.path.clone(),
            RainPathScope::Workspace(w) => w.resolve().join(&self.path),
            RainPathScope::Generated(id) => crate::config::global_config()
                .generated_directory()
                .join(id.to_string())
                .join(&self.path),
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum RainPathScope {
    Escaped,
    Workspace(Workspace),
    Generated(uuid::Uuid),
}

impl RainPathScope {
    pub fn new_path(self, path: impl AsRef<std::path::Path>) -> RainPath {
        RainPath {
            scope: self,
            path: path.as_ref().to_path_buf(),
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Workspace {
    Local(PathBuf),
    Remote(RemoteWorkspace),
}

impl Workspace {
    pub fn new_local_cwd() -> std::io::Result<Self> {
        Ok(Self::Local(std::env::current_dir()?))
    }

    pub fn new_path(&self, path: impl AsRef<std::path::Path>) -> RainPath {
        RainPath {
            scope: RainPathScope::Workspace(self.clone()),
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn resolve(&self) -> PathBuf {
        match self {
            Self::Local(p) => p.clone(),
            Self::Remote(r) => r.resolve(),
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum RemoteWorkspace {
    Git(GitWorkspace),
}

impl RemoteWorkspace {
    pub fn resolve(&self) -> PathBuf {
        todo!()
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct GitWorkspace {
    pub remote: String,
    pub reference: GitRef,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum GitRef {
    Commit(String),
    Tag(String),
    Branch(String),
}
