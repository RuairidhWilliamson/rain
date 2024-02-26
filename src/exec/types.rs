pub mod record;

use std::rc::Rc;

use crate::error::RainError;

use super::Executor;

#[derive(Debug, Clone, Copy)]
pub enum Type {
    Unit,
    Bool,
    Int,
    String,
    Path,
    Record,
    List,
    Function,
}

pub type DynValue = Rc<dyn Value>;

pub trait Value: std::fmt::Debug + std::fmt::Display {
    fn get_type(&self) -> Type;

    fn as_record(&self) -> Result<&record::Record, Type> {
        Err(self.get_type())
    }

    fn as_fn(&self) -> Result<&FnWrapper, Type> {
        Err(self.get_type())
    }
}

#[derive(Debug, Default)]
pub struct Unit;

impl Value for Unit {
    fn get_type(&self) -> Type {
        Type::Unit
    }
}

impl std::fmt::Display for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Unit")
    }
}

impl Value for bool {
    fn get_type(&self) -> Type {
        Type::Bool
    }
}

impl Value for u64 {
    fn get_type(&self) -> Type {
        Type::Int
    }
}

impl Value for String {
    fn get_type(&self) -> Type {
        Type::String
    }
}

pub struct FnWrapper(pub Box<dyn Fn(&mut Executor, &[DynValue]) -> Result<DynValue, RainError>>);

impl Value for FnWrapper {
    fn get_type(&self) -> Type {
        Type::Function
    }

    fn as_fn(&self) -> Result<&Self, Type> {
        Ok(&self)
    }
}

impl std::fmt::Debug for FnWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Function")
    }
}

impl std::fmt::Display for FnWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Function")
    }
}

impl FnWrapper {
    pub fn call(&self, executor: &mut Executor, args: &[DynValue]) -> Result<DynValue, RainError> {
        self.0(executor, args)
    }
}
