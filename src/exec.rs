mod stdlib;
pub mod types;

use crate::{
    ast::{
        declare::Declare, expr::Expr, fn_call::FnCall, fn_def::FnDef, item::Item, script::Script,
        stmt::Stmt,
    },
    error::RainError,
};

use self::types::RainValue;

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
        expected: &'static [types::RainType],
        actual: types::RainType,
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
    options: ExecuteOptions,
    call_stack: Vec<StackFrame>,
}

struct StackFrame {
    locals: types::record::Record,
}

impl Executor {
    pub fn new(options: ExecuteOptions) -> Self {
        let mut global_record = types::record::Record::default();
        global_record.insert(String::from("std"), RainValue::Record(stdlib::std_lib()));
        Self {
            global_record,
            options,
            call_stack: Vec::default(),
        }
    }
}

trait Executable {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, RainError>;
}

impl Executable for Script<'_> {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, RainError> {
        for stmt in &self.statements {
            stmt.execute(executor)?;
        }
        Ok(types::RainValue::Unit)
    }
}

impl Executable for Stmt<'_> {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, RainError> {
        match self {
            Self::Expr(expr) => expr.execute(executor),
            Self::Declare(declare) => declare.execute(executor),
            Self::FnDef(fndef) => fndef.execute(executor),
        }
    }
}

impl Executable for Expr<'_> {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, RainError> {
        match self {
            Self::Item(item) => item.execute(executor),
            Self::FnCall(fn_call) => fn_call.execute(executor),
            Self::BoolLiteral(value) => Ok(RainValue::Bool(*value)),
            Self::StringLiteral(value) => Ok(RainValue::String((*value).into())),
            Self::IfCondition(_) => todo!(),
        }
    }
}

impl Executable for FnCall<'_> {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, RainError> {
        let fn_value = self.item.execute(executor)?;
        let RainValue::Function(func) = fn_value else {
            return Err(RainError::new(
                ExecError::UnexpectedType {
                    expected: &[types::RainType::Function],
                    actual: fn_value.as_type(),
                },
                self.item.span,
            ));
        };

        let args = self
            .args
            .iter()
            .map(|a| a.execute(executor))
            .collect::<Result<Vec<RainValue>, RainError>>()?;
        func.call(executor, &args)
    }
}

impl Executable for Declare<'_> {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, RainError> {
        let value = self.value.execute(executor)?;
        executor
            .global_record
            .insert(self.name.name.to_owned(), value);
        Ok(types::RainValue::Unit)
    }
}

impl Executable for FnDef<'_> {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, RainError> {
        executor.global_record.insert(
            self.name.name.to_owned(),
            types::RainValue::Function(types::function::Function::new()),
        );
        Ok(types::RainValue::Unit)
    }
}

impl Executable for Item<'_> {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, RainError> {
        let (global, rest) = self.idents.split_first().unwrap();
        let mut record = executor
            .global_record
            .get(global.name)
            .ok_or(RainError::new(
                ExecError::UnknownVariable(global.name.to_owned()),
                global.span,
            ))?;
        for ident in rest {
            record = record
                .as_record()
                .map_err(|typ| {
                    RainError::new(
                        ExecError::UnexpectedType {
                            expected: &[types::RainType::Record],
                            actual: typ,
                        },
                        ident.span,
                    )
                })?
                .get(ident.name)
                .ok_or_else(|| {
                    RainError::new(ExecError::UnknownItem(String::from(ident.name)), ident.span)
                })?;
        }
        Ok(record.to_owned())
    }
}
