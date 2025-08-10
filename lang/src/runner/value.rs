use std::{
    fmt::{Debug, Display},
    hash::Hasher,
    sync::Arc,
};

use indexmap::IndexMap;

use crate::{
    afs::{
        absolute::AbsolutePathBuf, area::FileArea, dir::Dir, entry::FSEntryTrait as _, file::File,
    },
    ir::{DeclarationId, ModuleId},
};

use super::internal::InternalFunction;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Value {
    Unit,
    Boolean(bool),
    Integer(Arc<RainInteger>),
    String(Arc<String>),
    Function(DeclarationId),
    Module(ModuleId),
    FileArea(Arc<FileArea>),
    File(Arc<File>),
    EscapeFile(Arc<AbsolutePathBuf>),
    Dir(Arc<Dir>),
    Internal,
    InternalFunction(InternalFunction),
    List(Arc<RainList>),
    Record(Arc<RainRecord>),
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unit => f.write_str("unit"),
            Self::Boolean(b) => Display::fmt(&b, f),
            Self::Integer(rain_integer) => Display::fmt(&rain_integer, f),
            Self::String(s) => Debug::fmt(s, f),
            Self::Function(declaration_id) => Display::fmt(declaration_id, f),
            Self::Module(module_id) => Display::fmt(module_id, f),
            Self::FileArea(file_area) => Display::fmt(file_area, f),
            Self::File(file) => Display::fmt(file, f),
            Self::EscapeFile(path) => Display::fmt(&path.display(), f),
            Self::Dir(dir) => Display::fmt(dir, f),
            Self::Internal => f.write_str("internal"),
            Self::InternalFunction(internal_function) => Display::fmt(internal_function, f),
            Self::List(rain_list) => Display::fmt(rain_list, f),
            Self::Record(rain_record) => Display::fmt(rain_record, f),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum RainTypeId {
    Unit,
    Boolean,
    Integer,
    String,
    Function,
    Module,
    FileArea,
    File,
    EscapeFile,
    Dir,
    Internal,
    InternalFunction,
    List,
    Record,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct RainInteger(pub num_bigint::BigInt);

impl Display for RainInteger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
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
            Self::Function(_) => RainTypeId::Function,
            Self::Module(_) => RainTypeId::Module,
            Self::FileArea(_) => RainTypeId::FileArea,
            Self::File(_) => RainTypeId::File,
            Self::EscapeFile(_) => RainTypeId::EscapeFile,
            Self::Dir(_) => RainTypeId::Dir,
            Self::Internal => RainTypeId::Internal,
            Self::InternalFunction(_) => RainTypeId::InternalFunction,
            Self::List(_) => RainTypeId::List,
            Self::Record(_) => RainTypeId::Record,
        }
    }

    pub fn cache_pure(&self) -> bool {
        match self {
            Self::Internal
            | Self::InternalFunction(_)
            | Self::List(_)
            | Self::Record(_)
            | Self::Unit
            | Self::Boolean(_)
            | Self::Integer(_)
            | Self::String(_)
            | Self::Function(_)
            | Self::Module(_)
            | Self::FileArea(_)
            | Self::EscapeFile(_) => true,
            // TODO: Change
            Self::File(f) => match f.area() {
                FileArea::Generated(_) => true,
                FileArea::Local(_) => false,
            },
            Self::Dir(_) => false,
        }
    }

    pub fn find_areas(&self) -> Vec<&FileArea> {
        match self {
            Self::Unit
            | Self::Boolean(_)
            | Self::Integer(_)
            | Self::String(_)
            | Self::Function(_)
            | Self::Module(_)
            | Self::EscapeFile(_)
            | Self::Internal
            | Self::InternalFunction(_) => Vec::new(),
            Self::File(f) => vec![f.area()],
            Self::Dir(d) => vec![d.area()],
            Self::FileArea(file_area) => vec![file_area],
            Self::List(list) => list.0.iter().flat_map(|v| v.find_areas()).collect(),
            Self::Record(record) => record.0.iter().flat_map(|(_, v)| v.find_areas()).collect(),
        }
    }
}
