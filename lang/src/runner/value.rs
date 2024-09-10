use std::{
    any::{Any, TypeId},
    fmt::Debug,
    str::FromStr,
    sync::Arc,
};

use crate::ir::DeclarationId;

#[derive(Clone)]
pub struct RainValue {
    value: Arc<dyn RainValueInner>,
}

impl std::fmt::Debug for RainValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(f)
    }
}

impl RainValue {
    pub fn new<T: RainValueInner>(value: T) -> Self {
        Self {
            value: Arc::new(value),
        }
    }

    pub fn rain_type_id(&self) -> TypeId {
        (*self.value).type_id()
    }

    pub fn downcast<T: RainValueInner>(self) -> Option<Arc<T>> {
        if self.rain_type_id() == TypeId::of::<T>() {
            let ptr = Arc::into_raw(self.value);
            // Safety:
            // We have checked this is of the right type already
            Some(unsafe { Arc::from_raw(ptr.cast()) })
        } else {
            None
        }
    }
}

pub trait RainValueInner: Any + Debug + Send + Sync + RainHash {}

impl RainValueInner for () {}
impl RainValueInner for bool {}
impl RainValueInner for String {}

#[derive(Hash)]
pub struct RainInteger(pub num_bigint::BigInt);

impl RainValueInner for RainInteger {}
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

impl RainValueInner for RainFunction {}

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
