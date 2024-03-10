use std::rc::Rc;

use crate::{error::RainError, exec::Executor};

use super::RainValue;

#[derive(Debug, Clone)]
pub struct Function {
    implementation: Rc<FunctionImpl>,
}

impl Function {
    pub fn new() -> Self {
        Self {
            implementation: Rc::new(FunctionImpl::Local),
        }
    }

    pub fn new_external(fn_ptr: impl ExternalFnPtr + 'static) -> Self {
        Self {
            implementation: Rc::new(FunctionImpl::External(Box::new(fn_ptr))),
        }
    }

    pub fn call(
        &self,
        executor: &mut Executor,
        args: &[RainValue],
    ) -> Result<RainValue, RainError> {
        self.implementation.call(executor, args)
    }
}

impl std::fmt::Display for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Function")
    }
}

pub trait ExternalFnPtr: Fn(&mut Executor, &[RainValue]) -> Result<RainValue, RainError> {}
impl<F> ExternalFnPtr for F where F: Fn(&mut Executor, &[RainValue]) -> Result<RainValue, RainError> {}

enum FunctionImpl {
    Local,
    External(Box<dyn ExternalFnPtr>),
}

impl std::fmt::Debug for FunctionImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local => f.write_str("LocalFunctionImpl"),
            Self::External(_) => f.write_str("ExternalFunctionImpl"),
        }
    }
}

impl FunctionImpl {
    fn call(&self, executor: &mut Executor, args: &[RainValue]) -> Result<RainValue, RainError> {
        match self {
            // TODO: Implement this
            Self::Local => Ok(RainValue::Unit),
            Self::External(func) => func(executor, args),
        }
    }
}
