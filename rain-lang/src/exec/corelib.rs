use crate::{
    ast::{function_call::FnCall, Ast},
    error::RainError,
};

use super::{
    executor::Executor,
    types::{function::Function, record::Record, RainType, RainValue},
    ExecCF, ExecError, RuntimeError,
};

pub trait CoreHandler: std::fmt::Debug {
    fn print(&mut self, s: std::fmt::Arguments) {
        println!("{s}");
    }
}

#[derive(Debug, Clone)]
pub struct DefaultCoreHandler;

impl CoreHandler for DefaultCoreHandler {}

pub fn core_lib() -> Record {
    Record::new([
        (
            String::from("print"),
            RainValue::Function(Function::new_external(execute_print)),
        ),
        (
            String::from("error"),
            RainValue::Function(Function::new_external(execute_error)),
        ),
    ])
}

fn execute_print(
    executor: &mut Executor,
    args: &[RainValue],
    _fn_call: &FnCall<'_>,
) -> Result<RainValue, ExecCF> {
    struct Args<'a>(&'a [RainValue]);
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
    executor
        .global_executor()
        .core_handler
        .print(format_args!("{args}"));
    Ok(RainValue::Void)
}

fn execute_error(
    _executor: &mut Executor,
    args: &[RainValue],
    fn_call: &FnCall<'_>,
) -> Result<RainValue, ExecCF> {
    let [a] = args else {
        return Err(RainError::new(
            ExecError::IncorrectArgCount {
                expected: 1,
                actual: args.len(),
            },
            fn_call.span(),
        )
        .into());
    };
    let RainValue::String(s) = a else {
        return Err(RainError::new(
            ExecError::UnexpectedType {
                expected: &[RainType::String],
                actual: a.as_type(),
            },
            fn_call.args[0].span(),
        )
        .into());
    };
    Err(RuntimeError::new(s.to_string(), fn_call.span()).into())
}
