use std::{
    any::{Any, TypeId},
    fmt::Debug,
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

pub trait RainValueInner: Any + Debug + Send + Sync {}

impl RainValueInner for () {}
impl RainValueInner for bool {}
impl RainValueInner for isize {}
impl RainValueInner for String {}

#[derive(Debug)]
pub struct RainFunction {
    pub id: DeclarationId,
}

impl RainValueInner for RainFunction {}
