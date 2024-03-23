pub mod corelib;
pub mod types;

use std::path::PathBuf;

use crate::{
    ast::{
        block::Block, declare::Declare, expr::Expr, function_call::FnCall, function_def::FnDef,
        if_condition::IfCondition, item::Item, return_stmt::Return, script::Script,
        statement::Statement, statement_list::StatementList, Ast,
    },
    error::RainError,
};

use self::{
    corelib::{CoreHandler, DefaultCoreHandler},
    types::{RainType, RainValue},
};

#[derive(Debug)]
pub enum ExecError {
    UnknownVariable(String),
    UnknownItem(String),
    UnexpectedType {
        expected: &'static [types::RainType],
        actual: types::RainType,
    },
    IncorrectArgCount {
        expected: usize,
        actual: usize,
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

#[derive(Debug, Default)]
pub struct ExecutorBuilder {
    pub current_directory: PathBuf,
    pub core_handler: Option<Box<dyn CoreHandler>>,
    pub std_lib: Option<types::record::Record>,
    pub options: ExecuteOptions,
}

impl ExecutorBuilder {
    pub fn build(self) -> Executor {
        let current_directory = self.current_directory;
        let core_handler = self
            .core_handler
            .unwrap_or_else(|| Box::new(DefaultCoreHandler));
        let options = self.options;
        let mut global_record = types::record::Record::new([(
            String::from("core"),
            RainValue::Record(corelib::core_lib()),
        )]);
        if let Some(std_lib) = self.std_lib {
            global_record.insert(String::from("std"), RainValue::Record(std_lib));
        }

        Executor {
            current_directory,
            core_handler,
            global_record,
            options,
        }
    }
}

#[derive(Debug)]
pub struct Executor {
    pub core_handler: Box<dyn CoreHandler>,
    pub current_directory: PathBuf,
    global_record: types::record::Record,
    #[allow(dead_code)]
    options: ExecuteOptions,
}

pub trait Executable {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, RainError>;
}

impl Executable for Script<'static> {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, RainError> {
        self.statements.execute(executor)?;
        Ok(types::RainValue::Unit)
    }
}

impl Executable for Block<'static> {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, RainError> {
        self.stmts.execute(executor)
    }
}

impl Executable for StatementList<'static> {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, RainError> {
        let mut out = types::RainValue::Unit;
        for stmt in &self.statements {
            out = stmt.execute(executor)?;
            if let Statement::Return(_) = stmt {
                break;
            }
        }
        Ok(out)
    }
}

impl Executable for Statement<'static> {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, RainError> {
        match self {
            Self::Expr(expr) => expr.execute(executor),
            Self::Declare(declare) => declare.execute(executor),
            Self::FnDef(fndef) => fndef.execute(executor),
            Self::Return(ret) => ret.execute(executor),
        }
    }
}

impl Executable for Expr<'static> {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, RainError> {
        match self {
            Self::Item(item) => item.execute(executor),
            Self::FnCall(fn_call) => fn_call.execute(executor),
            Self::BoolLiteral(inner) => Ok(RainValue::Bool(inner.value)),
            Self::StringLiteral(inner) => Ok(RainValue::String((inner.value).into())),
            Self::IfCondition(inner) => inner.execute(executor),
            Self::Match(_) => todo!(),
        }
    }
}

impl Executable for FnCall<'static> {
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
        func.call(executor, &args, self)
    }
}

impl Executable for Declare<'static> {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, RainError> {
        let value = self.value.execute(executor)?;
        executor
            .global_record
            .insert(self.name.name.to_owned(), value);
        Ok(types::RainValue::Unit)
    }
}

impl Executable for FnDef<'static> {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, RainError> {
        executor.global_record.insert(
            self.name.name.to_owned(),
            types::RainValue::Function(types::function::Function::new(self.clone())),
        );
        Ok(types::RainValue::Unit)
    }
}

impl Executable for Item<'static> {
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

impl Executable for Return<'static> {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, RainError> {
        let value = self.expr.execute(executor)?;
        Ok(value)
    }
}

impl Executable for IfCondition<'static> {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, RainError> {
        let condition_value = self.condition.execute(executor)?;
        let RainValue::Bool(v) = condition_value else {
            return Err(RainError::new(
                ExecError::UnexpectedType {
                    expected: &[RainType::Bool],
                    actual: condition_value.as_type(),
                },
                self.condition.span(),
            ));
        };
        if v {
            return self.then_block.execute(executor);
        }
        if let Some(else_condition) = &self.else_condition {
            return else_condition.block.execute(executor);
        }
        Ok(RainValue::Unit)
    }
}
