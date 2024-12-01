use std::{
    any::{Any, TypeId},
    fmt::Debug,
    sync::Arc,
};

use super::{error::RunnerError, hash::RainHash};

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

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.value, &other.value)
            || self.value.rain_eq(other.value.as_ref())
    }
}

impl Eq for Value {}

impl std::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.value.rain_hash(state);
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

    fn any_type_id(&self) -> TypeId {
        (*self.value).type_id()
    }

    #[expect(unsafe_code)]
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
        downcast_ref(self.value.as_ref())
    }

    pub fn downcast_ref_error<T: ValueInner>(
        &self,
        expected: &'static [RainTypeId],
    ) -> Result<&T, RunnerError> {
        let actual = self.rain_type_id();
        self.downcast_ref::<T>()
            .ok_or(RunnerError::ExpectedType { actual, expected })
    }
}

#[expect(unsafe_code)]
fn downcast_ref<T: Any>(v: &dyn ValueInner) -> Option<&T> {
    if (*v).type_id() == TypeId::of::<T>() {
        let ptr = std::ptr::from_ref::<dyn ValueInner>(v);
        // Safety:
        // We have checked this is of the right type already
        Some(unsafe { &*ptr.cast::<T>() })
    } else {
        None
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

pub trait ValueInner: Any + Debug + Send + Sync + RainHash + RainEq {
    fn rain_type_id(&self) -> RainTypeId;
}

pub trait RainEq {
    fn rain_eq(&self, other: &dyn ValueInner) -> bool;
}

impl<T> RainEq for T
where
    T: Eq + 'static,
{
    fn rain_eq(&self, other: &dyn ValueInner) -> bool {
        let Some(o) = downcast_ref::<Self>(other) else {
            return false;
        };
        self.eq(o)
    }
}
