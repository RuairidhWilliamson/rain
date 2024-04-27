use std::rc::Rc;

use crate::{
    ast::{function_call::FnCall, function_def::FnDef, ident::Ident, Ast},
    error::RainError,
    exec::{execution::Execution, executor::Executor, ExecCF, ExecError},
    leaf::LeafSet,
    source::Source,
};

use super::{record::Record, RainValue};

pub type FunctionArguments<'a> = [(&'a Option<Ident>, RainValue)];

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
        args: &[(&Option<Ident>, RainValue)],
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
    Fn(&mut Executor, &FunctionArguments, Option<&FnCall>) -> Result<RainValue, ExecCF>
{
}
impl<F> ExternalFnPtr for F where
    F: Fn(&mut Executor, &FunctionArguments, Option<&FnCall>) -> Result<RainValue, ExecCF>
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
        args: &[(&Option<Ident>, RainValue)],
        fn_call: Option<&FnCall>,
    ) -> Result<RainValue, ExecCF> {
        match self {
            Self::Local(source, fn_def) => {
                if executor.call_depth > executor.base_executor.options.call_depth_limit {
                    return Err(ExecCF::RainError(RainError::new(
                        ExecError::CallDepthLimit,
                        fn_call.unwrap().span(),
                    )));
                }
                let named_args = args
                    .iter()
                    .filter_map(|(n, v)| n.as_ref().map(|n| (n, v)))
                    .filter(|(n, _)| fn_def.args.iter().any(|a| a.name.name == n.name))
                    .map(|(n, v)| dbg!((n.name.clone(), v.clone())));

                let positionals = fn_def
                    .args
                    .iter()
                    .zip(args.iter().filter(|(n, _)| n.is_none()))
                    .map(|(k, (_n, v))| (k.name.name.clone(), v.clone()));

                let locals = named_args.chain(positionals);
                let local_record = Record::new(locals);
                let mut new_executor = Executor {
                    base_executor: executor.base_executor,
                    script_executor: executor.script_executor,
                    local_record,
                    call_depth: executor.call_depth + 1,
                    leaves: LeafSet::default(),
                };
                let out = match fn_def.block.execute(&mut new_executor) {
                    Err(ExecCF::Return(v, _)) => Ok(v),
                    Err(ExecCF::RainError(err)) => Err(err.resolve(source.clone()).into()),
                    v => v,
                };
                executor.leaves.insert_set(&new_executor.leaves);
                out
            }
            Self::External(func) => func(executor, args, fn_call),
        }
    }
}
