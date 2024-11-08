use std::{
    any::{Any, TypeId},
    fmt::Debug,
    str::FromStr,
    sync::Arc,
};

use crate::{
    area::{File, FileArea},
    ir::{DeclarationId, ModuleId},
};

#[derive(Clone)]
pub struct Value {
    value: Arc<dyn ValueInner>,
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(f)
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(s) = self.downcast_ref::<String>() {
            std::fmt::Display::fmt(s, f)
        } else {
            std::fmt::Debug::fmt(self, f)
        }
    }
}

impl Value {
    pub fn new<T: ValueInner>(value: T) -> Self {
        Self {
            value: Arc::new(value),
        }
    }

    pub fn rain_type_id(&self) -> RainTypeId {
        self.value.rain_type_id()
    }

    pub fn any_type_id(&self) -> TypeId {
        (*self.value).type_id()
    }

    pub fn downcast<T: ValueInner>(self) -> Option<Arc<T>> {
        if self.any_type_id() == TypeId::of::<T>() {
            let ptr = Arc::into_raw(self.value);
            // Safety:
            // We have checked this is of the right type already
            Some(unsafe { Arc::from_raw(ptr.cast()) })
        } else {
            None
        }
    }

    pub fn downcast_ref<T: ValueInner>(&self) -> Option<&T> {
        if self.any_type_id() == TypeId::of::<T>() {
            let ptr = std::ptr::from_ref::<dyn ValueInner>(self.value.as_ref());
            // Safety:
            // We have checked this is of the right type already
            Some(unsafe { &*ptr.cast::<T>() })
        } else {
            None
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
    Internal,
    InternalFunction,
    List,
    Error,
}

pub trait ValueInner: Any + Debug + Send + Sync + RainHash {
    fn rain_type_id(&self) -> RainTypeId;
}

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

#[derive(Hash)]
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

#[derive(Debug, Hash)]
pub struct RainFunction {
    pub id: DeclarationId,
}

impl ValueInner for RainFunction {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::Function
    }
}

#[derive(Debug, Hash)]
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

#[derive(Debug, Hash)]
pub struct RainInternal;

impl ValueInner for RainInternal {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::Internal
    }
}

pub trait RainHash {
    fn hash(&self, state: &mut std::hash::DefaultHasher);
}

impl<T: std::hash::Hash> RainHash for T {
    fn hash(&self, state: &mut std::hash::DefaultHasher) {
        self.hash(state);
    }
}

impl RainHash for Value {
    fn hash(&self, state: &mut std::hash::DefaultHasher) {
        self.value.hash(state);
    }
}

pub struct RainList(pub Vec<Value>);

impl std::fmt::Debug for RainList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl RainHash for RainList {
    fn hash(&self, state: &mut std::hash::DefaultHasher) {
        for v in &self.0 {
            v.hash(state);
        }
    }
}

impl ValueInner for RainList {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::List
    }
}

#[derive(Debug, Hash)]
pub struct RainError(pub std::borrow::Cow<'static, str>);

impl ValueInner for RainError {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::Error
    }
}
