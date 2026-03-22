use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct User {
    pub id: i64,
    #[expect(dead_code)]
    pub email: Option<String>,
    pub name: String,
    #[expect(dead_code)]
    pub login: Option<String>,
    pub avatar_url: Option<String>,
}

impl User {
    pub fn is_admin(&self, config: &crate::Config) -> bool {
        self.id == config.allowed_user_id
    }

    pub fn display_name(&self) -> &str {
        &self.name
    }

    pub fn avatar_url(&self) -> &str {
        self.avatar_url
            .as_deref()
            .unwrap_or("https://placehold.co/400")
    }
}
