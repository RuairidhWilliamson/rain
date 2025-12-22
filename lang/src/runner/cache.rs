use std::{fmt::Display, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};

use crate::{afs::file::File, ir::DeclarationId, runner::dep_list::DepList};

use super::{internal::InternalFunction, value::Value};

pub trait CacheTrait {
    fn get(&self, key: &CacheKey) -> Option<CacheEntry>;
    fn put(&self, key: CacheKey, entry: CacheEntry);
    fn put_if_slow(&self, key: CacheKey, entry: CacheEntry);
    fn inspect_all(&self) -> Vec<String>;
    fn clean(&self);

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
    Prelude,
    Declaration {
        declaration: DeclarationId,
    },
    CallClosure {
        closure: super::value::Closure,
        args: Vec<Value>,
    },
    InternalFunction {
        func: InternalFunction,
        args: Vec<Value>,
    },
    Download {
        url: String,
    },
    Import {
        file: Arc<File>,
    },
}

impl Display for CacheKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Prelude => f.write_str("Prelude"),
            Self::Declaration { declaration } => f.write_fmt(format_args!("{declaration}")),
            Self::CallClosure { closure, args } => f.write_fmt(format_args!(
                "Closure({},{:?})({})",
                closure.module,
                closure.node,
                display_vec(args)
            )),
            Self::InternalFunction { func, args } => {
                f.write_fmt(format_args!("{func}({})", display_vec(args)))
            }
            Self::Download { url } => f.write_fmt(format_args!("Download({url})")),
            Self::Import { file } => f.write_fmt(format_args!("Import({file})")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub execution_time: Duration,
    pub expires: Option<DateTime<Utc>>,
    pub etag: Option<Vec<u8>>,
    pub deps: DepList,
    pub value: Value,
}
