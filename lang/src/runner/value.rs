use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    hash::Hasher,
    sync::Arc,
};

use alias::Alias;
use indexmap::IndexMap;

use crate::{
    afs::{
        FSEntryTrait as _,
        absolute::AbsolutePathBuf,
        area::FileAreaRef,
        generated::{area::GeneratedFSArea, dir::GeneratedDir, file::GeneratedFile},
        local::{area::LocalFSArea, dir::LocalDir, file::LocalFile},
    },
    ast::NodeId,
    ir::ModuleId,
    runner::internal::InternalFunction,
};

pub struct NamedValue {
    pub name: Arc<str>,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Value {
    Unit,
    Boolean(bool),
    Integer(Arc<RainInteger>),
    String(Arc<String>),
    Module(ModuleId),
    GeneratedFSArea(Arc<GeneratedFSArea>),
    LocalFSArea(Arc<LocalFSArea>),
    GeneratedFile(Arc<GeneratedFile>),
    LocalFile(Arc<LocalFile>),
    EscapeFile(Arc<AbsolutePathBuf>),
    GeneratedDir(Arc<GeneratedDir>),
    LocalDir(Arc<LocalDir>),
    Internal,
    InternalFunction(InternalFunction),
    List(Arc<RainList>),
    Record(Arc<RainRecord>),
    Closure(Closure),
    Type(RainTypeId),
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unit => f.write_str("unit"),
            Self::Boolean(b) => Display::fmt(&b, f),
            Self::Integer(rain_integer) => Display::fmt(&rain_integer, f),
            Self::String(s) => Debug::fmt(s, f),
            Self::Module(module_id) => Display::fmt(module_id, f),
            Self::GeneratedFSArea(area) => Display::fmt(area, f),
            Self::LocalFSArea(area) => Display::fmt(area, f),
            Self::GeneratedFile(file) => Display::fmt(file, f),
            Self::LocalFile(file) => Display::fmt(file, f),
            Self::EscapeFile(path) => Display::fmt(&path.display(), f),
            Self::GeneratedDir(dir) => Display::fmt(dir, f),
            Self::LocalDir(dir) => Display::fmt(dir, f),
            Self::Internal => f.write_str("internal"),
            Self::InternalFunction(internal_function) => Display::fmt(internal_function, f),
            Self::List(rain_list) => Display::fmt(rain_list, f),
            Self::Record(rain_record) => Display::fmt(rain_record, f),
            Self::Closure(closure) => Display::fmt(closure, f),
            Self::Type(typ) => Display::fmt(typ, f),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum RainTypeId {
    Unit,
    Boolean,
    Integer,
    String,
    Module,
    GeneratedFSArea,
    LocalFSArea,
    GeneratedFile,
    LocalFile,
    EscapeFile,
    GeneratedDir,
    LocalDir,
    Internal,
    InternalFunction,
    List,
    Record,
    Closure,
    Type,
}

impl std::fmt::Display for RainTypeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unit => "Unit",
            Self::Boolean => "Boolean",
            Self::Integer => "Integer",
            Self::String => "String",
            Self::Module => "Module",
            Self::GeneratedFSArea => "GeneratedFSArea",
            Self::LocalFSArea => "LocalFSArea",
            Self::GeneratedFile => "GeneratedFile",
            Self::LocalFile => "LocalFile",
            Self::EscapeFile => "EscapeFile",
            Self::GeneratedDir => "GeneratedDir",
            Self::LocalDir => "LocalDir",
            Self::Internal => "Internal",
            Self::InternalFunction => "InternalFunction",
            Self::List => "List",
            Self::Record => "Record",
            Self::Closure => "Closure",
            Self::Type => "Type",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct RainInteger(pub num_bigint::BigInt);

impl Display for RainInteger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl From<i32> for RainInteger {
    fn from(value: i32) -> Self {
        Self(num_bigint::BigInt::from(value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RainList(pub Vec<Value>);

impl std::hash::Hash for RainList {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for v in &self.0 {
            v.hash(state);
        }
    }
}

impl Display for RainList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[")?;
        let mut first = true;
        for v in &*self.0 {
            if !first {
                f.write_str(", ")?;
            }
            first = false;
            Display::fmt(v, f)?;
        }
        f.write_str("]")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RainRecord(pub IndexMap<String, Value>);

impl std::hash::Hash for RainRecord {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for (k, v) in &self.0 {
            k.hash(state);
            v.hash(state);
        }
    }
}

impl Display for RainRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("{")?;
        let mut first = true;
        for (k, v) in &self.0 {
            if !first {
                f.write_str(", ")?;
            }
            first = false;
            Display::fmt(k, f)?;
            f.write_str(": ")?;
            Display::fmt(v, f)?;
        }
        f.write_str("}")
    }
}

impl Value {
    pub const fn rain_type_id(&self) -> RainTypeId {
        match self {
            Self::Unit => RainTypeId::Unit,
            Self::Boolean(_) => RainTypeId::Boolean,
            Self::Integer(_) => RainTypeId::Integer,
            Self::String(_) => RainTypeId::String,
            Self::Module(_) => RainTypeId::Module,
            Self::GeneratedFSArea(_) => RainTypeId::GeneratedFSArea,
            Self::LocalFSArea(_) => RainTypeId::LocalFSArea,
            Self::GeneratedFile(_) => RainTypeId::GeneratedFile,
            Self::LocalFile(_) => RainTypeId::LocalFile,
            Self::EscapeFile(_) => RainTypeId::EscapeFile,
            Self::GeneratedDir(_) => RainTypeId::GeneratedDir,
            Self::LocalDir(_) => RainTypeId::LocalDir,
            Self::Internal => RainTypeId::Internal,
            Self::InternalFunction(_) => RainTypeId::InternalFunction,
            Self::List(_) => RainTypeId::List,
            Self::Record(_) => RainTypeId::Record,
            Self::Closure(_) => RainTypeId::Closure,
            Self::Type(_) => RainTypeId::Type,
        }
    }

    pub fn find_areas(&self) -> Vec<FileAreaRef<'_>> {
        match self {
            Self::Unit
            | Self::Boolean(_)
            | Self::Integer(_)
            | Self::String(_)
            | Self::Module(_)
            | Self::EscapeFile(_)
            | Self::Internal
            | Self::InternalFunction(_)
            | Self::Closure(_)
            | Self::Type(_) => Vec::new(),
            Self::GeneratedFile(f) => vec![f.area()],
            Self::LocalFile(f) => vec![f.area()],
            Self::GeneratedDir(d) => vec![d.area()],
            Self::LocalDir(d) => vec![d.area()],
            Self::GeneratedFSArea(area) => vec![area.as_ref().into()],
            Self::LocalFSArea(area) => vec![area.as_ref().into()],
            Self::List(list) => list.0.iter().flat_map(Self::find_areas).collect(),
            Self::Record(record) => record.0.iter().flat_map(|(_, v)| v.find_areas()).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Closure {
    pub captures: ClosureCaptures,
    pub module: ModuleId,
    pub node: NodeId,
}

impl std::fmt::Display for Closure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("Closure<{}, {:?}>", self.module, self.node))
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ClosureCaptures(pub Arc<HashMap<String, Value>>);

impl std::hash::Hash for ClosureCaptures {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // TODO: Optmisie this implementation, maybe you BTreeMap?
        let mut kv: Vec<_> = self.0.iter().collect();
        // Sorted to make the order stable
        kv.sort_unstable_by_key(|(name, _)| *name);
        for (k, v) in kv {
            k.hash(state);
            v.hash(state);
        }
    }
}

impl Alias for ClosureCaptures {}
