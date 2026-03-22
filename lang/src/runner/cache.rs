use std::{fmt::Display, time::Duration};

use chrono::{DateTime, Utc};

use crate::{
    afs::File,
    ast::NodeId,
    driver::FSTrait,
    runner::{
        LocalFileHashCache,
        dep_list::DepList,
        internal::InternalFunction,
        value::{ClosureCaptures, Value},
    },
};

pub trait CacheTrait {
    fn get(
        &self,
        key: &CacheKey,
        fs: &impl FSTrait,
        lfhc: &mut LocalFileHashCache,
    ) -> Option<CacheEntry>;
    fn put(&self, key: CacheKey, entry: CacheEntry);
    fn put_if_slow(&self, key: CacheKey, entry: CacheEntry);
    fn inspect_all(&self) -> Vec<String>;
    fn clean(&self);

    fn get_value(
        &self,
        key: &CacheKey,
        fs: &impl FSTrait,
        lfhc: &mut LocalFileHashCache,
    ) -> Option<Value> {
        self.get(key, fs, lfhc).map(|e| e.value)
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
    Embed,
    Declaration {
        module: File,
        name: String,
    },
    CallClosure {
        captures: ClosureCaptures,
        module: File,
        node: NodeId,
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
        file: File,
    },
}

impl Display for CacheKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Embed => f.write_str("Embed"),
            Self::Declaration { module: file, name } => {
                f.write_fmt(format_args!("Declaration({file}, {name})"))
            }
            Self::CallClosure {
                captures: _,
                module,
                node,
                args,
            } => f.write_fmt(format_args!(
                "Closure({},{:?})({})",
                module,
                node,
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
