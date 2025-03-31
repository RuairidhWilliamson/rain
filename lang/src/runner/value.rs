use std::{
    any::{Any, TypeId},
    fmt::{Debug, Display},
    sync::Arc,
};

use serde::ser::SerializeSeq as _;

use crate::afs::{area::FileArea, dir::Dir, file::File};

use super::{
    error::RunnerError,
    hash::RainHash,
    value_impl::{RainInteger, RainList, RainRecord, get_unit},
};

#[derive(Clone)]
pub struct Value {
    value: Arc<dyn ValueInner>,
}

impl Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.value, f)
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.value.rain_type_id() == RainTypeId::String {
            Debug::fmt(&self.value, f)
        } else {
            Display::fmt(&self.value, f)
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

    pub fn storeable(&self) -> bool {
        self.value.storeable()
    }

    pub fn cache_pure(&self) -> bool {
        self.value.cache_pure()
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

impl serde::Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_seq(Some(2))?;
        state.serialize_element(&self.rain_type_id())?;
        match self.rain_type_id() {
            RainTypeId::Unit => {
                state.serialize_element(&())?;
            }
            RainTypeId::Boolean => {
                state.serialize_element(self.downcast_ref::<bool>().unwrap())?;
            }
            RainTypeId::Integer => {
                state.serialize_element(self.downcast_ref::<RainInteger>().unwrap())?
            }
            RainTypeId::String => todo!(),
            RainTypeId::Function => todo!(),
            RainTypeId::Module => todo!(),
            RainTypeId::FileArea => {
                state.serialize_element(self.downcast_ref::<FileArea>().unwrap())?
            }
            RainTypeId::File => state.serialize_element(self.downcast_ref::<File>().unwrap())?,
            RainTypeId::Dir => state.serialize_element(self.downcast_ref::<Dir>().unwrap())?,
            RainTypeId::Internal => todo!(),
            RainTypeId::InternalFunction => todo!(),
            RainTypeId::List => {
                state.serialize_element(self.downcast_ref::<RainList>().unwrap())?
            }
            RainTypeId::Record => {
                state.serialize_element(self.downcast_ref::<RainRecord>().unwrap())?
            }
        }
        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_struct("rain_value", &["type", "value"], ValueVisitor)
    }
}

struct ValueVisitor;

impl<'de> serde::de::Visitor<'de> for ValueVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a rain value")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let r#type: Option<RainTypeId> = seq.next_element()?;
        match r#type {
            Some(RainTypeId::Unit) => {
                let _ = seq.next_element::<()>()?;
                Ok(get_unit())
            }
            Some(RainTypeId::Boolean) => Ok(Value::new(seq.next_element::<bool>()?.unwrap())),
            Some(RainTypeId::Integer) => {
                Ok(Value::new(seq.next_element::<RainInteger>()?.unwrap()))
            }
            Some(RainTypeId::String) => todo!(),
            Some(RainTypeId::Function) => todo!(),
            Some(RainTypeId::Module) => todo!(),
            Some(RainTypeId::FileArea) => todo!(),
            Some(RainTypeId::File) => Ok(Value::new(seq.next_element::<File>()?.unwrap())),
            Some(RainTypeId::Dir) => todo!(),
            Some(RainTypeId::Internal) => todo!(),
            Some(RainTypeId::InternalFunction) => todo!(),
            Some(RainTypeId::List) => todo!(),
            Some(RainTypeId::Record) => Ok(Value::new(seq.next_element::<RainRecord>()?.unwrap())),
            None => todo!(),
        }
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut r#type: Option<RainTypeId> = None;
        let mut value: Option<File> = None;
        while let Some(k) = map.next_key()? {
            match k {
                "type" => {
                    if r#type.is_some() {
                        return Err(serde::de::Error::duplicate_field("type"));
                    }
                    r#type = Some(map.next_value()?);
                }
                "value" => {
                    if value.is_some() {
                        return Err(serde::de::Error::duplicate_field("value"));
                    }
                    value = Some(map.next_value()?);
                }
                field => return Err(serde::de::Error::unknown_field(field, &["type", "value"])),
            }
        }
        let r#type = r#type.ok_or_else(|| serde::de::Error::missing_field("type"))?;
        assert_eq!(r#type, RainTypeId::File);
        let value = value.ok_or_else(|| serde::de::Error::missing_field("value"))?;
        Ok(Value::new(value))
    }
}

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

#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RainTypeId {
    Unit,
    Boolean,
    Integer,
    String,
    Function,
    Module,
    FileArea,
    File,
    Dir,
    Internal,
    InternalFunction,
    List,
    Record,
}

pub trait ValueInner: Any + Debug + Display + Send + Sync + RainHash + RainEq {
    fn rain_type_id(&self) -> RainTypeId;

    fn storeable(&self) -> bool {
        true
    }

    fn cache_pure(&self) -> bool {
        true
    }
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
