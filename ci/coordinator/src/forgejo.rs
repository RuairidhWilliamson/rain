use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ForgejoPushEvent {
    pub after: String,
    pub repository: ForgejoRepository,
}

#[derive(Debug, Deserialize)]
pub struct ForgejoRepository {
    pub owner: ForgejoOwner,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct ForgejoOwner {
    pub login: String,
}
