mod stdlib;
pub mod types;

use std::rc::Rc;

use crate::{
    ast::{
        declare::Declare, expr::Expr, fn_call::FnCall, fn_def::FnDef, item::Item, stmt::Stmt,
        Script,
    },
    error::RainError,
};

use self::types::DynValue;

pub fn execute(script: &Script<'_>, options: ExecuteOptions) -> Result<(), RainError> {
    let mut executor = Executor::new(options);
    script.execute(&mut executor)?;
    Ok(())
}

#[derive(Debug)]
pub enum ExecError {
    UnknownVariable(String),
    UnknownItem(String),
    UnexpectedType {
        expected: types::Type,
        actual: types::Type,
    },
}

impl std::fmt::Display for ExecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

#[derive(Debug, Default, Clone)]
pub struct ExecuteOptions {
    pub sealed: bool,
}

#[derive(Default)]
pub struct Executor {
    global_record: types::record::Record,
    #[allow(unused)]
    options: ExecuteOptions,
}

impl Executor {
    pub fn new(options: ExecuteOptions) -> Self {
        let mut global_record = types::record::Record::default();
        global_record.insert(String::from("std"), Rc::new(stdlib::std_lib()));
        Self {
            global_record,
            options,
        }
    }
}

trait Executable {
    fn execute(&self, executor: &mut Executor) -> Result<DynValue, RainError>;
}

impl Executable for Script<'_> {
    fn execute(&self, executor: &mut Executor) -> Result<DynValue, RainError> {
        for stmt in &self.statements {
            stmt.execute(executor)?;
        }
        Ok(Rc::new(types::Unit))
    }
}

impl Executable for Stmt<'_> {
    fn execute(&self, executor: &mut Executor) -> Result<DynValue, RainError> {
        match self {
            Self::Expr(expr) => expr.execute(executor),
            Self::Declare(declare) => declare.execute(executor),
            Self::FnDef(fndef) => fndef.execute(executor),
        }
    }
}

impl Executable for Expr<'_> {
    fn execute(&self, executor: &mut Executor) -> Result<DynValue, RainError> {
        match self {
            Self::Item(item) => item.execute(executor),
            Self::FnCall(fn_call) => fn_call.execute(executor),
            Self::BoolLiteral(value) => Ok(Rc::new(*value)),
            Self::StringLiteral(value) => Ok(Rc::new(String::from(*value))),
        }
    }
}

impl Executable for FnCall<'_> {
    fn execute(&self, executor: &mut Executor) -> Result<DynValue, RainError> {
        let fn_dyn_value = self.item.execute(executor)?;
        let fn_value = fn_dyn_value.as_fn().map_err(|typ| {
            RainError::new(
                ExecError::UnexpectedType {
                    expected: types::Type::Function,
                    actual: typ,
                },
                self.item.span,
            )
        })?;
        let args = self
            .args
            .iter()
            .map(|a| a.execute(executor))
            .collect::<Result<Vec<DynValue>, RainError>>()?;
        fn_value.call(executor, &args)
    }
}

impl Executable for Declare<'_> {
    fn execute(&self, executor: &mut Executor) -> Result<DynValue, RainError> {
        let value = self.value.execute(executor)?;
        executor.global_record.insert(self.name.to_owned(), value);
        Ok(Rc::new(types::Unit))
    }
}

impl Executable for FnDef<'_> {
    fn execute(&self, executor: &mut Executor) -> Result<DynValue, RainError> {
        // executor.global_record.insert(self.name.to_owned());
        todo!()
    }
}

impl Executable for Item<'_> {
    fn execute(&self, executor: &mut Executor) -> Result<DynValue, RainError> {
        let (global, rest) = self.idents.split_first().unwrap();
        let mut record = executor
            .global_record
            .get(global.name)
            .ok_or(RainError::new(
                ExecError::UnknownVariable(global.name.to_owned()),
                self.span,
            ))?;
        for ident in rest {
            record = record
                .as_record()
                .map_err(|typ| {
                    RainError::new(
                        ExecError::UnexpectedType {
                            expected: types::Type::Record,
                            actual: typ,
                        },
                        self.span,
                    )
                })?
                .get(ident.name)
                .ok_or_else(|| {
                    RainError::new(ExecError::UnknownItem(String::from(ident.name)), self.span)
                })?;
        }
        Ok(record.to_owned())
    }
}
