use std::{marker::PhantomData, str::FromStr, sync::LazyLock};

use indexmap::IndexMap;

use crate::{
    afs::{area::FileArea, dir::Dir, entry::FSEntryTrait as _, file::File},
    ir::{DeclarationId, ModuleId},
};

use super::{
    hash::RainHash,
    value::{RainTypeId, Value, ValueInner},
};

static UNIT: LazyLock<Value> = LazyLock::new(|| super::Value::new(RainUnit(PhantomData)));

pub fn get_unit() -> Value {
    UNIT.clone()
}

#[derive(Hash, PartialEq, Eq)]
pub struct RainUnit(PhantomData<()>);

impl std::fmt::Debug for RainUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("unit")
    }
}

impl ValueInner for RainUnit {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::Unit
    }
}

impl std::fmt::Display for RainUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("unit")
    }
}

impl ValueInner for bool {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::Boolean
    }
}

impl ValueInner for String {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::String
    }
}

#[derive(Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RainInteger(pub num_bigint::BigInt);

impl ValueInner for RainInteger {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::Integer
    }
}

impl std::fmt::Display for RainInteger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl std::fmt::Debug for RainInteger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, f)
    }
}
impl FromStr for RainInteger {
    type Err = num_bigint::ParseBigIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        num_bigint::BigInt::from_str(s).map(Self)
    }
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct RainFunction {
    pub id: DeclarationId,
}

impl ValueInner for RainFunction {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::Function
    }
}

impl std::fmt::Display for RainFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.id, f)
    }
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct Module {
    pub id: ModuleId,
}

impl ValueInner for Module {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::Module
    }
}

impl std::fmt::Display for Module {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.id, f)
    }
}

impl ValueInner for FileArea {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::FileArea
    }
}

impl ValueInner for File {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::File
    }

    fn cache_pure(&self) -> bool {
        match self.area() {
            // Files outside of rain's control are not pure since they are mutable, making determining if they are the same file as before more complicated
            // TODO: Implement escape/local file caching
            FileArea::Escape | FileArea::Local(_) => false,
            // Files in a generated area are considered pure since they are considered by rain to be immutable
            FileArea::Generated(_) => true,
        }
    }
}

impl ValueInner for Dir {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::Dir
    }

    fn cache_pure(&self) -> bool {
        // TODO: Really think about if this is correct
        false
    }
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct RainInternal;

impl ValueInner for RainInternal {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::Internal
    }
}

impl std::fmt::Display for RainInternal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("internal")
    }
}

#[derive(Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RainList(pub Vec<Value>);

impl std::fmt::Debug for RainList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl ValueInner for RainList {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::List
    }
}

impl std::fmt::Display for RainList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[")?;
        let mut first = true;
        for v in &self.0 {
            if !first {
                f.write_str(", ")?;
            }
            first = false;
            std::fmt::Display::fmt(v, f)?;
        }
        f.write_str("]")
    }
}

#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RainRecord(pub IndexMap<String, Value>);

impl ValueInner for RainRecord {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::Record
    }
}

impl std::fmt::Display for RainRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("{")?;
        let mut first = true;
        for (k, v) in &self.0 {
            if !first {
                f.write_str(", ")?;
            }
            first = false;
            f.write_str(k)?;
            f.write_str(": ")?;
            std::fmt::Display::fmt(v, f)?;
        }
        f.write_str("}")
    }
}

impl RainHash for RainRecord {
    fn rain_hash(&self, state: &mut dyn std::hash::Hasher) {
        for (k, v) in &self.0 {
            k.rain_hash(state);
            v.rain_hash(state);
        }
    }
}
