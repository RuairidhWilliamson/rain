use std::{
    any::{Any, TypeId},
    fmt::Debug,
};

#[derive(Debug)]
pub struct RainValue {
    value: Box<dyn RainValueInner>,
}

impl RainValue {
    pub fn new<T: RainValueInner>(value: T) -> Self {
        Self {
            value: Box::new(value),
        }
    }

    pub fn downcast<T: RainValueInner>(self) -> Option<Box<T>> {
        if (*self.value).type_id() == TypeId::of::<T>() {
            let ptr = Box::into_raw(self.value);
            // Safety:
            // We have checked this is of the right type already
            Some(unsafe { Box::from_raw(ptr.cast()) })
        } else {
            None
        }
    }
}

pub trait RainValueInner: Any + Debug + Send + Sync {}

impl RainValueInner for bool {}
impl RainValueInner for isize {}
impl RainValueInner for String {}
