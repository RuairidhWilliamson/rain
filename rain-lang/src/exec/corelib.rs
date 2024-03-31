use std::{path::PathBuf, rc::Rc, str::FromStr};

use crate::{
    ast::{function_call::FnCall, Ast},
    error::RainError,
    exec::types::path::Path,
};

use super::{
    execution::Execution,
    executor::{Executor, ScriptExecutor},
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
        (
            String::from("import"),
            RainValue::Function(Function::new_external(execute_import)),
        ),
        (
            String::from("path"),
            RainValue::Function(Function::new_external(execute_path)),
        ),
    ])
}

fn execute_print(
    executor: &mut Executor,
    args: &[RainValue],
    _fn_call: Option<&FnCall<'_>>,
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
    executor.core_handler().print(format_args!("{args}"));
    Ok(RainValue::Void)
}

fn execute_error(
    _executor: &mut Executor,
    args: &[RainValue],
    fn_call: Option<&FnCall<'_>>,
) -> Result<RainValue, ExecCF> {
    let [a] = args else {
        return Err(RainError::new(
            ExecError::IncorrectArgCount {
                expected: (1..=1).into(),
                actual: args.len(),
            },
            fn_call.unwrap().span(),
        )
        .into());
    };
    let RainValue::String(s) = a else {
        return Err(RainError::new(
            ExecError::UnexpectedType {
                expected: &[RainType::String],
                actual: a.as_type(),
            },
            fn_call.unwrap().args[0].span(),
        )
        .into());
    };
    Err(RuntimeError::new(s.to_string(), fn_call.unwrap().span()).into())
}

fn execute_import(
    executor: &mut Executor,
    args: &[RainValue],
    fn_call: Option<&FnCall<'_>>,
) -> Result<RainValue, ExecCF> {
    let [a] = args else {
        return Err(RainError::new(
            ExecError::IncorrectArgCount {
                expected: (1..=1).into(),
                actual: args.len(),
            },
            fn_call.unwrap().span(),
        )
        .into());
    };
    let RainValue::Path(p) = a else {
        return Err(RainError::new(
            ExecError::UnexpectedType {
                expected: &[RainType::String],
                actual: a.as_type(),
            },
            fn_call.unwrap().args[0].span(),
        )
        .into());
    };
    let script_path = p.absolute();
    tracing::info!("importing {script_path:?}");
    // TODO: Don't leak this, properly track the lifetime
    let source = std::fs::read_to_string(&script_path).unwrap().leak();
    let mut token_stream = crate::tokens::peek_stream::PeekTokenStream::new(source);
    let script = crate::ast::script::Script::parse_stream(&mut token_stream)?;
    let mut new_script_executor = ScriptExecutor {
        global_record: Record::default(),
        current_directory: script_path.parent().unwrap().to_path_buf(),
    };
    let mut new_executor = Executor::new(executor.base_executor, &mut new_script_executor);
    Execution::execute(&script, &mut new_executor)?;
    Ok(new_script_executor.global_record.into())
}

fn execute_path(
    executor: &mut Executor,
    args: &[RainValue],
    fn_call: Option<&FnCall<'_>>,
) -> Result<RainValue, ExecCF> {
    let [a] = args else {
        return Err(RainError::new(
            ExecError::IncorrectArgCount {
                expected: (1..=1).into(),
                actual: args.len(),
            },
            fn_call.unwrap().span(),
        )
        .into());
    };
    let RainValue::String(s) = a else {
        return Err(RainError::new(
            ExecError::UnexpectedType {
                expected: &[RainType::String],
                actual: a.as_type(),
            },
            fn_call.unwrap().span(),
        )
        .into());
    };
    Ok(RainValue::Path(Rc::new(Path {
        path: PathBuf::from_str(s).unwrap(),
        current_directory: executor.current_directory().to_path_buf(),
    })))
}
