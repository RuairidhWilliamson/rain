use std::rc::Rc;

use crate::{
    ast::{function_call::FnCall, function_def::FnDef},
    exec::{execution::Execution, executor::Executor, ExecCF},
    source::Source,
};

use super::{record::Record, RainValue};

#[derive(Debug, Clone)]
pub struct Function {
    implementation: Rc<FunctionImpl>,
}

impl Function {
    pub fn new(source: Source, fn_def: FnDef) -> Self {
        Self {
            implementation: Rc::new(FunctionImpl::Local(source, fn_def)),
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
        fn_call: Option<&FnCall>,
    ) -> Result<RainValue, ExecCF> {
        self.implementation.call(executor, args, fn_call)
    }
}

impl std::fmt::Display for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Function")
    }
}

pub trait ExternalFnPtr:
    Fn(&mut Executor, &[RainValue], Option<&FnCall>) -> Result<RainValue, ExecCF>
{
}
impl<F> ExternalFnPtr for F where
    F: Fn(&mut Executor, &[RainValue], Option<&FnCall>) -> Result<RainValue, ExecCF>
{
}

enum FunctionImpl {
    Local(Source, FnDef),
    External(Box<dyn ExternalFnPtr>),
}

impl std::fmt::Debug for FunctionImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local(_, _) => f.write_str("LocalFunctionImpl"),
            Self::External(_) => f.write_str("ExternalFunctionImpl"),
        }
    }
}

impl FunctionImpl {
    fn call(
        &self,
        executor: &mut Executor,
        args: &[RainValue],
        fn_call: Option<&FnCall>,
    ) -> Result<RainValue, ExecCF> {
        match self {
            Self::Local(source, fn_def) => {
                let local_record = Record::new(
                    fn_def
                        .args
                        .iter()
                        .zip(args)
                        .map(|(k, v)| (String::from(&k.name.name), v.clone())),
                );
                let mut executor = Executor {
                    base_executor: executor.base_executor,
                    script_executor: executor.script_executor,
                    local_record,
                };
                match fn_def.block.execute(&mut executor) {
                    Err(ExecCF::Return(v)) => Ok(v),
                    Err(ExecCF::RainError(err)) => Err(err.resolve(source.clone()).into()),
                    v => v,
                }
            }
            Self::External(func) => func(executor, args, fn_call),
        }
    }
}
