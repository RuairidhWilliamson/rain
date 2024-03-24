use crate::{ast::function_call::FnCall, error::RainError};

use super::{
    executor::Executor,
    types::{function::Function, record::Record, RainValue},
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
    let mut core_lib = Record::default();
    core_lib.insert(
        String::from("print"),
        RainValue::Function(Function::new_external(execute_print)),
    );
    core_lib
}

fn execute_print(
    executor: &mut Executor,
    args: &[RainValue],
    _fn_call: &FnCall<'_>,
) -> Result<RainValue, RainError> {
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
