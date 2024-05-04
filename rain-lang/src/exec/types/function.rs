use std::rc::Rc;

use crate::{
    ast::{function_call::FnCall, function_def::FnDef, ident::Ident, Ast},
    cache::{CacheEntry, FunctionCallCacheKey},
    error::RainError,
    exec::{execution::Execution, executor::FunctionExecutor, ExecCF, ExecError},
    leaf::LeafSet,
    source::Source,
};

use super::RainValue;

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

    pub fn new_external(func: impl ExternalFn + 'static) -> Self {
        Self {
            implementation: Rc::new(FunctionImpl::External(Box::new(func))),
        }
    }

    pub fn call(
        &self,
        executor: &mut FunctionExecutor,
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

pub trait ExternalFn {
    fn call(
        &self,
        executor: &mut FunctionExecutor,
        args: &FunctionArguments,
        call: Option<&FnCall>,
    ) -> Result<RainValue, ExecCF>;
}

enum FunctionImpl {
    Local(Source, FnDef),
    External(Box<dyn ExternalFn>),
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
        executor: &mut FunctionExecutor,
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
                    .map(|(n, v)| (n.name.clone(), v.clone()));

                let positionals = fn_def
                    .args
                    .iter()
                    .zip(args.iter().filter(|(n, _)| n.is_none()))
                    .map(|(k, (_n, v))| (k.name.name.clone(), v.clone()));
                let k = FunctionCallCacheKey::default();
                if let Some(v) = executor.base_executor.cache.get(&k) {
                    tracing::info!("cache hit");
                    executor.leaves.insert_set(&v.leaves);
                    return Ok(v.value);
                }
                tracing::info!("cache miss");

                let locals = named_args.chain(positionals);
                let local_record = locals.collect();
                let mut new_executor = FunctionExecutor {
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
                let leaves = new_executor.leaves;
                executor.leaves.insert_set(&leaves);
                if let Ok(v) = &out {
                    executor.base_executor.cache.put(
                        k,
                        CacheEntry {
                            value: v.clone(),
                            leaves,
                        },
                    );
                }
                out
            }
            Self::External(func) => func.call(executor, args, fn_call),
        }
    }
}
