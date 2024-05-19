use std::rc::Rc;

use ordered_hash_map::OrderedHashMap;

use crate::{
    ast::{declaration::FunctionDeclaration, function_call::FnCall, ident::Ident, Ast},
    cache::CacheEntry,
    error::RainError,
    exec::{execution::Execution, ExecCF, ExecError},
    executor::{script::ScriptExecutor, Executor},
    leaf::LeafSet,
};

use super::RainValue;

pub type FunctionArguments<'a> = [(&'a Option<Ident>, RainValue)];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    implementation: Rc<FunctionImpl>,
}

impl Function {
    pub fn new(script_executor: ScriptExecutor, function: FunctionDeclaration) -> Self {
        Self {
            implementation: Rc::new(FunctionImpl::Local(script_executor, function)),
        }
    }

    pub fn new_external(func: impl ExternalFn + 'static) -> Self {
        Self {
            implementation: Rc::new(FunctionImpl::External(Box::new(func))),
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

pub trait ExternalFn {
    fn call(
        &self,
        executor: &mut Executor,
        args: &FunctionArguments,
        call: Option<&FnCall>,
    ) -> Result<RainValue, ExecCF>;
}

enum FunctionImpl {
    Local(ScriptExecutor, FunctionDeclaration),
    External(Box<dyn ExternalFn>),
}

impl PartialEq for FunctionImpl {
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}

impl Eq for FunctionImpl {}

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
        let key = None;
        if let Some(entry) = executor.base_executor.cache.get_option(&key) {
            executor.leaves.insert_set(&entry.leaves);
            return entry.value;
        }
        if executor.call_depth > executor.base_executor.options.call_depth_limit {
            return Err(ExecCF::RainError(RainError::new(
                ExecError::CallDepthLimit,
                fn_call.unwrap().span(),
            )));
        }
        let entry = match self {
            Self::Local(script_executor, fn_def) => {
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
                let locals = named_args.chain(positionals);
                let local_record = locals.collect();
                let mut new_executor = Executor {
                    base_executor: executor.base_executor,
                    script_executor,
                    local_record,
                    call_depth: executor.call_depth + 1,
                    leaves: LeafSet::default(),
                };
                let out = fn_def.block.execute(&mut new_executor);
                let out = match out {
                    Err(ExecCF::Return(v, _)) => Ok(v),
                    Err(ExecCF::RainError(err)) => {
                        Err(err.resolve(script_executor.source().clone()).into())
                    }
                    v => v,
                };
                CacheEntry {
                    value: out,
                    leaves: new_executor.leaves,
                }
            }
            Self::External(func) => {
                let mut new_executor = Executor {
                    base_executor: executor.base_executor,
                    script_executor: executor.script_executor,
                    local_record: OrderedHashMap::default(),
                    call_depth: executor.call_depth + 1,
                    leaves: LeafSet::default(),
                };
                let out = func.call(&mut new_executor, args, fn_call);
                CacheEntry {
                    value: out,
                    leaves: new_executor.leaves,
                }
            }
        };
        executor.base_executor.cache.put_option(key, &entry);

        executor.leaves.insert_set(&entry.leaves);
        entry.value
    }
}
