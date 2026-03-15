pub mod repository;
pub mod repository_host;
pub mod run;

use std::path::PathBuf;

use anyhow::{Context as _, Result, anyhow};
use reqwest::Url;
use secrecy::{ExposeSecret as _, SecretString};
use sqlx::{ConnectOptions as _, postgres::PgConnectOptions};

pub struct DbConfig {
    pub url: Url,
    pub password_file: Option<PathBuf>,
}

async fn load_password(config: &DbConfig) -> Result<Option<SecretString>> {
    if let Some(password_file) = &config.password_file {
        return Ok(Some(
            tokio::fs::read_to_string(password_file)
                .await
                .context("cannot read DATABASE_PASSWORD_FILE")?
                .into(),
        ));
    }
    Err(anyhow!("set DATABASE_PASSWORD_FILE"))
}

#[derive(Clone)]
pub struct Db {
    pub pool: sqlx::PgPool,
}

impl Db {
    pub async fn new(config: DbConfig, application_name: &str) -> Result<Self> {
        let db_password = load_password(&config).await?;
        let mut options = PgConnectOptions::from_url(&config.url)?;
        if let Some(db_password) = db_password {
            options = options.password(db_password.expose_secret());
        }
        options = options.application_name(application_name);
        let pool = sqlx::PgPool::connect_with(options).await?;
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
