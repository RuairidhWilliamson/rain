#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Dep {
    /// Marks any calls that depend on this to be uncacheable
    Uncacheable,
    /// Marks any calls that depend on this to depend on a local area
    // TODO: Specify the local area
    LocalArea,
    /// Marks any calls that depend on this to depend on the escaped environment
    Escape,
    /// Marks any calls that depend on this to depend on the secret
    // TODO: Specify the secret name
    Secret,
    /// Marks the call as depending on the calling module
    CallingModule,
    /// This prints so should not be cached
    Print,
}

impl Dep {
    pub fn is_propogated_in_closure(&self) -> bool {
        match self {
            Dep::CallingModule => false,
            _ => true,
        }
    }

    pub fn is_intra_run_stable(&self) -> bool {
        match self {
            Self::Uncacheable | Self::CallingModule | Self::Print => false,
            Self::LocalArea | Self::Escape | Self::Secret => true,
        }
    }

    pub fn is_inter_run_stable(&self) -> bool {
        false
    }
}
