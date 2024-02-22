use std::collections::HashMap;

use crate::{
    ast::{
        expr::{Expr, FnCall, Item},
        stmt::{Declare, Stmt},
        Script,
    },
    span::Span,
};

pub fn execute(script: &Script<'_>, options: ExecuteOptions) -> Result<(), ExecError> {
    let mut executor = Executor {
        options,
        ..Executor::default()
    };
    script.execute(&mut executor)?;
    Ok(())
}

#[derive(Debug, Default, Clone)]
pub struct ExecuteOptions {
    pub sealed: bool,
}

#[derive(Default)]
struct Executor {
    variables: HashMap<String, ExecValue>,
    #[allow(unused)]
    options: ExecuteOptions,
}

impl Executor {
    fn declare_variable(&mut self, name: &str, value: ExecValue) {
        self.variables.insert(String::from(name), value);
    }

    fn lookup_variable(&self, item: &Item<'_>) -> Option<ExecValue> {
        let (tli, rest) = item.idents.split_first()?;
        match *tli {
            "std" => self.lookup_std(rest),
            v => self.variables.get(v).cloned(),
        }
    }

    fn lookup_std(&self, path: &[&str]) -> Option<ExecValue> {
        match path {
            ["print"] => Some(ExecValue::Function(Function::StdFunc(StdFunc::Print))),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
enum ExecValue {
    Unit,
    Bool(bool),
    String(String),
    Function(Function),
}

impl std::fmt::Display for ExecValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unit => f.write_str("Unit"),
            Self::Bool(val) => f.write_str(if *val { "true" } else { "false" }),
            Self::String(val) => f.write_str(val),
            Self::Function(_) => f.write_str("Function"),
        }
    }
}

#[derive(Debug)]
pub enum ExecError {
    UnknownItem(Span),
    CannotCallNonFunction(Span),
}

impl ExecError {
    pub fn span(&self) -> Span {
        match self {
            ExecError::UnknownItem(span) => *span,
            ExecError::CannotCallNonFunction(span) => *span,
        }
    }
}

impl std::fmt::Display for ExecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self, f)
    }
}

#[derive(Debug, Clone)]
enum Function {
    StdFunc(StdFunc),
}

#[derive(Debug, Clone)]
enum StdFunc {
    Print,
}

impl Function {
    fn execute(&self, args: &[ExecValue]) {
        match self {
            Function::StdFunc(std_function) => std_function.execute(args),
        }
    }
}

impl StdFunc {
    fn execute(&self, args: &[ExecValue]) {
        match self {
            StdFunc::Print => self.execute_print(args),
        }
    }

    fn execute_print(&self, args: &[ExecValue]) {
        struct Args<'a>(&'a [ExecValue]);
        impl std::fmt::Display for Args<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let Some((first, rest)) = self.0.split_first() else {
                    return Ok(());
                };
                first.fmt(f)?;
                for a in rest {
                    a.fmt(f)?;
                    f.write_str(" ")?;
                }
                Ok(())
            }
        }
        let args = Args(args);
        println!("{args}");
    }
}

trait Executable {
    fn execute(&self, executor: &mut Executor) -> Result<ExecValue, ExecError>;
}

impl Executable for Script<'_> {
    fn execute(&self, executor: &mut Executor) -> Result<ExecValue, ExecError> {
        for stmt in &self.statements {
            stmt.execute(executor)?;
        }
        Ok(ExecValue::Unit)
    }
}

impl Executable for Stmt<'_> {
    fn execute(&self, executor: &mut Executor) -> Result<ExecValue, ExecError> {
        match self {
            Self::Expr(expr) => expr.execute(executor),
            Self::Declare(declare) => declare.execute(executor),
        }
    }
}

impl Executable for Expr<'_> {
    fn execute(&self, executor: &mut Executor) -> Result<ExecValue, ExecError> {
        match self {
            Self::Item(item) => item.execute(executor),
            Self::FnCall(fn_call) => fn_call.execute(executor),
            Self::BoolLiteral(value) => Ok(ExecValue::Bool(*value)),
            Self::StringLiteral(value) => Ok(ExecValue::String(String::from(*value))),
        }
    }
}

impl Executable for FnCall<'_> {
    fn execute(&self, executor: &mut Executor) -> Result<ExecValue, ExecError> {
        let Some(fn_value) = executor.lookup_variable(&self.item) else {
            return Err(ExecError::UnknownItem(self.span));
        };
        let ExecValue::Function(function) = fn_value else {
            return Err(ExecError::CannotCallNonFunction(self.item.span));
        };
        let args = self
            .args
            .iter()
            .map(|a| a.execute(executor))
            .collect::<Result<Vec<ExecValue>, ExecError>>()?;
        function.execute(&args);
        Ok(ExecValue::Unit)
    }
}

impl Executable for Declare<'_> {
    fn execute(&self, executor: &mut Executor) -> Result<ExecValue, ExecError> {
        let value = self.value.execute(executor)?;
        executor.declare_variable(self.name, value);
        Ok(ExecValue::Unit)
    }
}

impl Executable for Item<'_> {
    fn execute(&self, executor: &mut Executor) -> Result<ExecValue, ExecError> {
        executor
            .lookup_variable(self)
            .ok_or(ExecError::UnknownItem(self.span))
    }
}
