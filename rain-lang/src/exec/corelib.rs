use std::rc::Rc;

use crate::{
    ast::{function_call::FnCall, script::Script, Ast},
    error::RainError,
    source::Source,
    tokens::peek_stream::PeekTokenStream,
};

use super::{
    executor::FunctionExecutor,
    types::{
        function::{ExternalFn, Function, FunctionArguments},
        record::Record,
        RainType, RainValue,
    },
    ExecCF, ExecError, RuntimeError,
};

pub trait CoreHandler: std::fmt::Debug {
    #[allow(clippy::print_stderr)]
    fn print(&mut self, s: std::fmt::Arguments) {
        eprintln!("{s}");
    }
}

#[derive(Debug, Clone)]
pub struct DefaultCoreHandler;

impl CoreHandler for DefaultCoreHandler {}

pub fn core_lib() -> Record {
    Record::new([
        (
            String::from("print"),
            RainValue::Function(Function::new_external(CorePrint)),
        ),
        (
            String::from("error"),
            RainValue::Function(Function::new_external(CoreError)),
        ),
        (
            String::from("import"),
            RainValue::Function(Function::new_external(CoreImport)),
        ),
        (
            String::from("path"),
            RainValue::Function(Function::new_external(CorePath)),
        ),
        (
            String::from("file"),
            RainValue::Function(Function::new_external(CoreFile)),
        ),
    ])
}

#[derive(PartialEq, Eq)]
struct CorePrint;

impl ExternalFn for CorePrint {
    fn call(
        &self,
        executor: &mut FunctionExecutor,
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
}

#[derive(PartialEq, Eq)]
struct CoreError;

impl ExternalFn for CoreError {
    fn call(
        &self,
        _executor: &mut FunctionExecutor,
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
}

#[derive(PartialEq, Eq)]
struct CoreImport;

impl ExternalFn for CoreImport {
    fn call(
        &self,
        executor: &mut FunctionExecutor,
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
        Ok(RainValue::Script(Rc::new(script)))
    }
}

#[derive(PartialEq, Eq)]
struct CorePath;

impl ExternalFn for CorePath {
    fn call(
        &self,
        _executor: &mut FunctionExecutor,
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
}

#[derive(PartialEq, Eq)]
struct CoreFile;

impl ExternalFn for CoreFile {
    fn call(
        &self,
        executor: &mut FunctionExecutor,
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
}
