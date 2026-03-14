pub mod repository;
pub mod repository_host;
pub mod run;

use std::path::PathBuf;

use anyhow::{Context as _, Result, anyhow};
use secrecy::{ExposeSecret as _, SecretString};

pub struct DbConfig {
    pub host: String,
    pub name: String,
    pub user: String,
    pub password: Option<SecretString>,
    pub password_file: Option<PathBuf>,
}

async fn load_password(config: &DbConfig) -> Result<SecretString> {
    if let Some(password) = &config.password {
        return Ok(password.clone());
    }
    if let Some(password_file) = &config.password_file {
        return Ok(tokio::fs::read_to_string(password_file)
            .await
            .context("cannot read DB_PASSWORD_FILE")?
            .into());
    }
    Err(anyhow!("set DB_PASSWORD or DB_PASSWORD_FILE"))
}

#[derive(Clone)]
pub struct Db {
    pub pool: sqlx::PgPool,
}

impl Db {
    pub async fn new(config: DbConfig) -> Result<Self> {
        let db_password = load_password(&config).await?;
        let pool = sqlx::PgPool::connect_with(
            sqlx::postgres::PgConnectOptions::new()
                .host(&config.host)
                .username(&config.user)
                .password(db_password.expose_secret())
                .database(&config.name),
        )
        .await?;
        Ok(Self { pool })
    }
}

#[expect(async_fn_in_trait)]
pub trait Resource: Sized {
    type Id;

    async fn get(db: &Db, id: Self::Id) -> Result<WithId<Self>>;
}

pub struct WithId<T: Resource> {
    pub id: T::Id,
    pub resource: T,
}
