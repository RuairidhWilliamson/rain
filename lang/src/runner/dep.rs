#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Dep {
    /// Marks any calls that depend on this to be uncacheable
    Uncacheable,
    /// Marks any calls that depend on this to depend on a local area
    // TODO: Specify the local area
    LocalArea,
    /// Marks any calls that depend on this to depend on the escaped environment
    EscapeArea,
}
