#![allow(clippy::print_stdout)]

use std::{
    ffi::OsStr,
    hash::Hasher as _,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result, anyhow};
use postgres::{
    NoTls,
    types::{FromSql, ToSql},
};
use rustc_stable_hash::{FromStableHash, SipHasher128Hash};

#[derive(Debug, serde::Deserialize)]
struct Config {
    db_host: String,
    db_name: String,
    db_user: String,
    db_password_file: PathBuf,
    migrations_dir: PathBuf,
}

fn main() -> Result<()> {
    let config = envy::from_env::<Config>()?;
    let mut db = postgres::Config::new()
        .host(&config.db_host)
        .dbname(&config.db_name)
        .user(&config.db_user)
        .password(std::fs::read_to_string(config.db_password_file)?)
        .connect(NoTls)?;

    let mut tx = db.transaction()?;
    tx.execute(
        "CREATE TABLE IF NOT EXISTS migrations (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            hash BYTEA NOT NULL
        )",
        &[],
    )?;
    let mut migrations_iter = get_migrations(&config.migrations_dir)?
        .into_iter()
        .peekable();

    while let Some(peek) = migrations_iter.peek() {
        let Some(row) =
            tx.query_opt("SELECT name, hash FROM migrations WHERE id=$1", &[&peek.id])?
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
        if let Some(row) = tx.query_opt("SELECT name FROM migrations WHERE id=$1", &[&m.id])? {
            let name: &str = row.get("name");
            return Err(anyhow!(
                "migration id {} already exists with name {}",
                m.id,
                name
            ));
        }
        tx.batch_execute(&m.sql)?;
        tx.execute(
            "INSERT INTO migrations (id, name, hash) VALUES ($1, $2, $3)",
            &[&m.id, &m.name, &m.hash],
        )?;
        println!("Migration {} {} performed", m.id, m.name);
    }

    tx.commit()?;
    println!("All migrations checked/completed successfully");
    Ok(())
}

fn get_migrations(dir: &Path) -> Result<Vec<Migration>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension() != Some(OsStr::new("sql")) {
            continue;
        }
        let file_name = path
            .file_stem()
            .context("get file stem failed")?
            .to_str()
            .context("non-utf8 migration name")?;
        let (id, name) = file_name
            .split_once('_')
            .context("invalid migration name {file_name:?}, must include underscore")?;
        let id: i32 = id.parse()?;
        let name = name.to_owned();
        let sql = std::fs::read_to_string(path)?;
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

#[derive(Debug, PartialEq, Eq, FromSql, ToSql)]
#[postgres(transparent)]
pub struct Hash128(Vec<u8>);

impl FromStableHash for Hash128 {
    type Hash = SipHasher128Hash;

    fn from(SipHasher128Hash(hash): SipHasher128Hash) -> Self {
        let bytes = hash.map(u64::to_le_bytes).concat();
        Self(bytes)
    }
}
