use std::{path::PathBuf, rc::Rc};

pub mod function;
pub mod record;

#[derive(Debug, Clone, enum_kinds::EnumKind)]
#[enum_kind(RainType)]
pub enum RainValue {
    Void,
    Bool(bool),
    String(Rc<str>),
    Path(Rc<PathBuf>),
    Record(record::Record),
    List(Rc<[RainValue]>),
    Function(function::Function),
}

impl RainValue {
    pub fn as_type(&self) -> RainType {
        RainType::from(self)
    }

    pub fn as_record(&self) -> Result<&record::Record, RainType> {
        let Self::Record(record) = self else {
            return Err(self.as_type());
        };
        Ok(record)
    }
}

impl std::fmt::Display for RainValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RainValue::Void => f.write_str("Void"),
            RainValue::Bool(b) => b.fmt(f),
            RainValue::String(s) => s.fmt(f),
            RainValue::Path(p) => std::fmt::Debug::fmt(&p, f),
            RainValue::Record(r) => r.fmt(f),
            RainValue::List(_) => f.write_str("List"),
            RainValue::Function(func) => func.fmt(f),
        }
    }
}
