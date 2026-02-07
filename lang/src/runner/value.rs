use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    hash::Hasher,
    sync::Arc,
};

use indexmap::IndexMap;

use crate::{
    afs::{
        FSEntryTrait as _,
        absolute::AbsolutePathBuf,
        area::{FileArea, FileAreaRef},
        generated::{dir::GeneratedDir, file::GeneratedFile},
        local::{dir::LocalDir, file::LocalFile},
    },
    ast::NodeId,
    ir::ModuleId,
};

use super::internal::InternalFunction;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Value {
    Unit,
    Boolean(bool),
    Integer(Arc<RainInteger>),
    String(Arc<String>),
    Module(ModuleId),
    FileArea(Arc<FileArea>),
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
            Self::FileArea(file_area) => Display::fmt(file_area, f),
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
    FileArea,
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
            Self::FileArea => "FileArea",
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
    pub fn rain_type_id(&self) -> RainTypeId {
        match self {
            Self::Unit => RainTypeId::Unit,
            Self::Boolean(_) => RainTypeId::Boolean,
            Self::Integer(_) => RainTypeId::Integer,
            Self::String(_) => RainTypeId::String,
            Self::Module(_) => RainTypeId::Module,
            Self::FileArea(_) => RainTypeId::FileArea,
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
            Self::FileArea(file_area) => vec![file_area.as_ref().as_area_ref()],
            Self::List(list) => list.0.iter().flat_map(Self::find_areas).collect(),
            Self::Record(record) => record.0.iter().flat_map(|(_, v)| v.find_areas()).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Closure {
    pub captures: Arc<HashMap<String, Value>>,
    pub module: ModuleId,
    pub node: NodeId,
}

impl std::fmt::Display for Closure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("Closure<{}, {:?}>", self.module, self.node))
    }
}

impl std::hash::Hash for Closure {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for (k, v) in self.captures.iter() {
            k.hash(state);
            v.hash(state);
        }
        self.module.hash(state);
        self.node.hash(state);
    }
}
