use std::rc::Rc;

pub mod file;
pub mod function;
pub mod path;
pub mod record;

#[derive(Debug, Clone, enum_kinds::EnumKind)]
#[enum_kind(RainType)]
pub enum RainValue {
    Void,
    Lazy,
    Bool(bool),
    String(Rc<str>),
    Path(Rc<path::Path>),
    File(Rc<file::File>),
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

impl From<record::Record> for RainValue {
    fn from(rec: record::Record) -> Self {
        Self::Record(rec)
    }
}

impl std::fmt::Display for RainValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Void => f.write_str("Void"),
            Self::Lazy => f.write_str("Lazy"),
            Self::Bool(b) => b.fmt(f),
            Self::String(s) => s.fmt(f),
            Self::Path(p) => std::fmt::Debug::fmt(&p, f),
            Self::File(file) => std::fmt::Debug::fmt(&file, f),
            Self::Record(r) => r.fmt(f),
            Self::List(_) => f.write_str("List"),
            Self::Function(func) => func.fmt(f),
        }
    }
}
