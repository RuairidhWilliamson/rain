use std::{fmt::Display, time::Duration};

use chrono::{DateTime, Utc};

use crate::ir::DeclarationId;

use super::{dep::Dep, internal::InternalFunction, value::Value};

pub trait CacheTrait {
    fn get(&self, key: &CacheKey) -> Option<CacheEntry>;
    fn put(
        &self,
        key: CacheKey,
        execution_time: Duration,
        etag: Option<String>,
        deps: &[Dep],
        value: Value,
    );
    fn inspect_all(&self) -> Vec<String>;

    fn get_value(&self, key: &CacheKey) -> Option<Value> {
        self.get(key).map(|e| e.value)
    }
}

fn display_vec<T: Display>(v: &Vec<T>) -> String {
    let mut s = String::new();
    let mut first = true;
    for e in v {
        if !first {
            s.push(',');
        }
        first = false;
        s.push_str(&e.to_string());
    }
    s
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CacheKey {
    Declaration {
        declaration: DeclarationId,
        args: Vec<Value>,
    },
    InternalFunction {
        func: InternalFunction,
        args: Vec<Value>,
    },
    Download {
        url: String,
    },
}

impl CacheKey {
    pub fn pure(&self) -> bool {
        match self {
            Self::InternalFunction { func: _, args }
            | Self::Declaration {
                declaration: _,
                args,
            } => args.iter().all(Value::cache_pure),
            Self::Download { url: _ } => true,
        }
    }
}

impl Display for CacheKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Declaration { declaration, args } => {
                f.write_fmt(format_args!("{declaration}({})", display_vec(args)))
            }
            Self::InternalFunction { func, args } => {
                f.write_fmt(format_args!("{func}({})", display_vec(args)))
            }
            Self::Download { url } => f.write_fmt(format_args!("Download({url})")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub execution_time: Duration,
    pub expires: Option<DateTime<Utc>>,
    pub etag: Option<String>,
    pub deps: Vec<Dep>,
    pub value: Value,
}

pub enum CacheStrategy {
    Always,
    Never,
}
