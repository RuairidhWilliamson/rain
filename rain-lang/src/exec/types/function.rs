use std::rc::Rc;

use crate::{
    ast::{fn_call::FnCall, fn_def::FnDef},
    error::RainError,
    exec::{Executable, Executor},
};

use super::RainValue;

#[derive(Debug, Clone)]
pub struct Function {
    implementation: Rc<FunctionImpl>,
}

impl Function {
    pub fn new(fn_def: FnDef<'static>) -> Self {
        Self {
            implementation: Rc::new(FunctionImpl::Local(fn_def)),
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
        fn_call: &FnCall<'_>,
    ) -> Result<RainValue, RainError> {
        self.implementation.call(executor, args, fn_call)
    }
}

impl std::fmt::Display for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Function")
    }
}

pub trait ExternalFnPtr:
    Fn(&mut Executor, &[RainValue], &FnCall<'_>) -> Result<RainValue, RainError>
{
}
impl<F> ExternalFnPtr for F where
    F: Fn(&mut Executor, &[RainValue], &FnCall<'_>) -> Result<RainValue, RainError>
{
}

enum FunctionImpl {
    Local(FnDef<'static>),
    External(Box<dyn ExternalFnPtr>),
}

impl std::fmt::Debug for FunctionImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local(_) => f.write_str("LocalFunctionImpl"),
            Self::External(_) => f.write_str("ExternalFunctionImpl"),
        }
    }
}

impl FunctionImpl {
    fn call(
        &self,
        executor: &mut Executor,
        args: &[RainValue],
        fn_call: &FnCall<'_>,
    ) -> Result<RainValue, RainError> {
        match self {
            Self::Local(fn_def) => fn_def.block.execute(executor),
            Self::External(func) => func(executor, args, fn_call),
        }
    }
}
