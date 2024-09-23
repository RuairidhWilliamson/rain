use std::{
    any::{Any, TypeId},
    fmt::Debug,
    str::FromStr,
    sync::Arc,
};

use crate::ir::{DeclarationId, ModuleId};

#[derive(Clone)]
pub struct RainValue {
    value: Arc<dyn RainValueInner>,
}

impl std::fmt::Debug for RainValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(f)
    }
}

impl std::fmt::Display for RainValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(s) = self.downcast_ref::<String>() {
            std::fmt::Display::fmt(s, f)
        } else {
            std::fmt::Debug::fmt(self, f)
        }
    }
}

impl RainValue {
    pub fn new<T: RainValueInner>(value: T) -> Self {
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

    pub fn downcast<T: RainValueInner>(self) -> Option<Arc<T>> {
        if self.any_type_id() == TypeId::of::<T>() {
            let ptr = Arc::into_raw(self.value);
            // Safety:
            // We have checked this is of the right type already
            Some(unsafe { Arc::from_raw(ptr.cast()) })
        } else {
            None
        }
    }

    pub fn downcast_ref<T: RainValueInner>(&self) -> Option<&T> {
        if self.any_type_id() == TypeId::of::<T>() {
            let ptr = std::ptr::from_ref::<dyn RainValueInner>(self.value.as_ref());
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
    Internal,
    InternalFunction,
}

pub trait RainValueInner: Any + Debug + Send + Sync + RainHash {
    fn rain_type_id(&self) -> RainTypeId;
}

impl RainValueInner for () {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::Unit
    }
}
impl RainValueInner for bool {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::Boolean
    }
}
impl RainValueInner for String {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::String
    }
}

#[derive(Hash)]
pub struct RainInteger(pub num_bigint::BigInt);

impl RainValueInner for RainInteger {
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

impl RainValueInner for RainFunction {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::Function
    }
}

#[derive(Debug, Hash)]
pub struct RainModule {
    pub id: ModuleId,
}

impl RainValueInner for RainModule {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::Module
    }
}

#[derive(Debug, Hash)]
pub struct RainInternal;

impl RainValueInner for RainInternal {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::Internal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RainInternalFunction {
    Print,
    Import,
}

impl RainValueInner for RainInternalFunction {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::InternalFunction
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

impl RainHash for RainValue {
    fn hash(&self, state: &mut std::hash::DefaultHasher) {
        self.value.hash(state);
    }
}
