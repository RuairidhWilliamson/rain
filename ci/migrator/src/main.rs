#![allow(clippy::print_stdout)]

use std::{
    ffi::OsStr,
    hash::Hasher as _,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result, anyhow};
use rustc_stable_hash::{FromStableHash, SipHasher128Hash};
use secrecy::{ExposeSecret as _, SecretString};
use sqlx::{Connection as _, Row as _};

#[derive(Debug, serde::Deserialize)]
struct Config {
    db_host: String,
    db_name: String,
    db_user: String,
    db_password: Option<SecretString>,
    db_password_file: Option<PathBuf>,
    migrations_dir: PathBuf,
}

async fn load_password(config: &Config) -> Result<SecretString> {
    if let Some(password) = &config.db_password {
        return Ok(password.clone());
    }
    if let Some(password_file) = &config.db_password_file {
        return Ok(tokio::fs::read_to_string(password_file)
            .await
            .context("cannot read DB_PASSWORD_FILE")?
            .into());
    }
    Err(anyhow!("set DB_PASSWORD or DB_PASSWORD_FILE"))
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    let config = envy::from_env::<Config>()?;
    let db_password = load_password(&config).await?;
    let mut conn = sqlx::postgres::PgConnection::connect_with(
        &sqlx::postgres::PgConnectOptions::new()
            .host(&config.db_host)
            .username(&config.db_user)
            .password(db_password.expose_secret())
            .database(&config.db_name),
    )
    .await?;

    let mut tx = conn.begin().await?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS migrations (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            hash BYTEA NOT NULL
        )",
    )
    .execute(&mut *tx)
    .await?;
    let mut migrations_iter = get_migrations(&config.migrations_dir)
        .await?
        .into_iter()
        .peekable();

    while let Some(peek) = migrations_iter.peek() {
        let Some(row) = sqlx::query("SELECT name, hash FROM migrations WHERE id=$1")
            .bind(peek.id)
            .fetch_optional(&mut *tx)
            .await?
        else {
            break;
        };
        let Some(m) = migrations_iter.next() else {
            unreachable!("peek checked")
        };
        let name: &str = row.get("name");
        let hash: Hash128 = row.get("hash");
        if m.name != name {
            return Err(anyhow!("name does not match for {name}"));
        }
        if m.hash != hash {
            return Err(anyhow!("hash does not match for {name}"));
        }
        println!("Verified {name}");
    }

    for m in migrations_iter {
        if let Some(row) = sqlx::query("SELECT name FROM migrations WHERE id=$1")
            .bind(m.id)
            .fetch_optional(&mut *tx)
            .await?
        {
            let name: &str = row.get("name");
            return Err(anyhow!(
                "migration id {} already exists with name {}",
                m.id,
                name
            ));
        }

        match sqlx::raw_sql(&m.sql).execute(&mut *tx).await {
            Ok(_) => {}
            Err(err) => {
                println!("Error performing migration {} with name {}", m.id, m.name);
                println!("{err:#}");
                return Err(anyhow!("performing migration failed"));
            }
        }
        sqlx::query("INSERT INTO migrations (id, name, hash) VALUES ($1, $2, $3)")
            .bind(m.id)
            .bind(&m.name)
            .bind(m.hash)
            .execute(&mut *tx)
            .await?;
        println!("Migration {} {} performed", m.id, m.name);
    }

    tx.commit().await?;
    println!("All migrations checked/completed successfully");
    Ok(())
}

async fn get_migrations(dir: &Path) -> Result<Vec<Migration>> {
    let mut out = Vec::new();
    let mut files = tokio::fs::read_dir(dir).await?;
    while let Some(entry) = files.next_entry().await? {
        let path = entry.path();
        if path.extension() != Some(OsStr::new("sql")) {
            continue;
        }
        let file_name = path
            .file_stem()
            .context("get file stem failed")?
            .to_str()
            .context("non-utf8 migration name")?;
        let (id, name) = file_name.split_once('_').context(anyhow!(
            "invalid migration name {file_name:?}, must include underscore"
        ))?;
        let id: i32 = id.parse()?;
        let name = name.to_owned();
        let sql = tokio::fs::read_to_string(path).await?;
        let mut hasher = rustc_stable_hash::StableSipHasher128::new();
        hasher.write(sql.as_bytes());
        let hash: Hash128 = hasher.finish();
        out.push(Migration {
            id,
            name,
            sql,
            hash,
        });
    }
    out.sort_by_key(|m| m.id);
    Ok(out)
}

pub struct Migration {
    pub id: i32,
    pub name: String,
    pub sql: String,
    pub hash: Hash128,
}

#[derive(Debug, PartialEq, Eq, sqlx::Type)]
#[sqlx(transparent)]
pub struct Hash128(Vec<u8>);

impl FromStableHash for Hash128 {
    type Hash = SipHasher128Hash;

    fn from(SipHasher128Hash(hash): SipHasher128Hash) -> Self {
        let bytes = hash.map(u64::to_le_bytes).concat();
        Self(bytes)
    }
}
