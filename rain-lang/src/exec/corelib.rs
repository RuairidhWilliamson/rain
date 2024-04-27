use std::rc::Rc;

use crate::{
    ast::{function_call::FnCall, script::Script, Ast},
    error::RainError,
    source::Source,
    tokens::peek_stream::PeekTokenStream,
};

use super::{
    execution::Execution,
    executor::{Executor, ScriptExecutor},
    types::{
        function::{Function, FunctionArguments},
        record::Record,
        RainType, RainValue,
    },
    ExecCF, ExecError, RuntimeError,
};

pub trait CoreHandler: std::fmt::Debug {
    #[allow(clippy::print_stdout)]
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
        (
            String::from("file"),
            RainValue::Function(Function::new_external(execute_file)),
        ),
    ])
}

fn execute_print(
    executor: &mut Executor,
    args: &FunctionArguments,
    _fn_call: Option<&FnCall>,
) -> Result<RainValue, ExecCF> {
    struct Args<'a>(&'a FunctionArguments<'a>);
    impl std::fmt::Display for Args<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let Some(((_, first), rest)) = self.0.split_first() else {
                return Ok(());
            };
            first.fmt(f)?;
            for (_, a) in rest {
                f.write_str(" ")?;
                a.fmt(f)?;
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
    args: &FunctionArguments,
    fn_call: Option<&FnCall>,
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
    let (_, RainValue::String(s)) = a else {
        return Err(RainError::new(
            ExecError::UnexpectedType {
                expected: &[RainType::String],
                actual: a.1.as_type(),
            },
            fn_call.unwrap().args[0].span(),
        )
        .into());
    };
    Err(RuntimeError::new(s.to_string(), fn_call.unwrap().span()).into())
}

fn execute_import(
    executor: &mut Executor,
    args: &FunctionArguments,
    fn_call: Option<&FnCall>,
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
    let (_, RainValue::Path(script_path)) = a else {
        return Err(RainError::new(
            ExecError::UnexpectedType {
                expected: &[RainType::String],
                actual: a.1.as_type(),
            },
            fn_call.unwrap().args[0].span(),
        )
        .into());
    };
    let script_file = executor
        .script_executor
        .source
        .path
        .directory()
        .unwrap()
        .join(script_path.as_ref());
    tracing::info!("importing {script_file}");
    let source = Source::new(&script_file).unwrap();
    let mut token_stream = PeekTokenStream::new(&source.source);
    let script =
        Script::parse_stream(&mut token_stream).map_err(|err| err.resolve(source.clone()))?;
    let mut new_script_executor = ScriptExecutor::new(source.clone());
    let mut new_executor = Executor::new(executor.base_executor, &mut new_script_executor);
    Execution::execute(&script, &mut new_executor)
        .map_err(|err| err.map_resolve(|err| err.resolve(source).into()))?;
    tracing::info!("corelib import leaves {:?}", new_executor.leaves);
    executor.leaves.insert_set(&new_executor.leaves);
    Ok(new_script_executor.global_record.into())
}

fn execute_path(
    _executor: &mut Executor,
    args: &FunctionArguments,
    fn_call: Option<&FnCall>,
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
    let (_, RainValue::String(s)) = a else {
        return Err(RainError::new(
            ExecError::UnexpectedType {
                expected: &[RainType::String],
                actual: a.1.as_type(),
            },
            fn_call.unwrap().span(),
        )
        .into());
    };
    Ok(RainValue::Path(s.clone()))
}

fn execute_file(
    executor: &mut Executor,
    args: &FunctionArguments,
    fn_call: Option<&FnCall>,
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
    let (_, RainValue::Path(s)) = a else {
        return Err(RainError::new(
            ExecError::UnexpectedType {
                expected: &[RainType::Path],
                actual: a.1.as_type(),
            },
            fn_call.unwrap().span(),
        )
        .into());
    };
    let file = executor
        .script_executor
        .source
        .path
        .directory()
        .unwrap()
        .join(s.as_ref());
    Ok(RainValue::File(Rc::new(file)))
}
