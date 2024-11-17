use std::str::FromStr;

use crate::{
    area::{File, FileArea},
    ir::{DeclarationId, ModuleId},
};

use super::{
    hash::RainHash,
    value::{RainTypeId, Value, ValueInner},
};

impl ValueInner for () {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::Unit
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

#[derive(Hash, PartialEq, Eq)]
pub struct RainInteger(pub num_bigint::BigInt);

impl ValueInner for RainInteger {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::Integer
    }
}

impl std::fmt::Debug for RainInteger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
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

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct Module {
    pub id: ModuleId,
}

impl ValueInner for Module {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::Module
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
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct RainInternal;

impl ValueInner for RainInternal {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::Internal
    }
}

pub struct RainList(pub Vec<Value>);

impl std::fmt::Debug for RainList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl RainHash for RainList {
    fn rain_hash(&self, state: &mut dyn std::hash::Hasher) {
        for v in &self.0 {
            v.rain_hash(state);
        }
    }
}

impl ValueInner for RainList {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::List
    }
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct RainError(pub std::borrow::Cow<'static, str>);

impl ValueInner for RainError {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::Error
    }
}
