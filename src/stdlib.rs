mod escape;

use std::path::PathBuf;
use std::rc::Rc;
use std::str::FromStr;

use rain_lang::ast::function_call::FnCall;
use rain_lang::ast::Ast;
use rain_lang::error::RainError;
use rain_lang::exec::executor::Executor;
use rain_lang::exec::types::RainType;
use rain_lang::exec::types::{function::Function, record::Record, RainValue};
use rain_lang::exec::{ExecCF, ExecError};

pub fn std_lib() -> Record {
    Record::new([
        (
            String::from("run"),
            RainValue::Function(Function::new_external(execute_run)),
        ),
        (
            String::from("download"),
            RainValue::Function(Function::new_external(execute_download)),
        ),
        (
            String::from("path"),
            RainValue::Function(Function::new_external(execute_path)),
        ),
        (
            String::from("escape"),
            RainValue::Record(escape::std_escape_lib()),
        ),
    ])
}

fn execute_run(
    executor: &mut Executor,
    args: &[RainValue],
    fn_call: Option<&FnCall<'_>>,
) -> Result<RainValue, ExecCF> {
    let Some((program, args)) = args.split_first() else {
        return Err(RainError::new(
            ExecError::IncorrectArgCount {
                expected: 1,
                actual: 0,
            },
            fn_call.unwrap().span(),
        )
        .into());
    };
    let RainValue::Path(program) = program else {
        return Err(RainError::new(
            ExecError::UnexpectedType {
                expected: &[RainType::Path],
                actual: program.as_type(),
            },
            fn_call.unwrap().span(),
        )
        .into());
    };
    let mut cmd = std::process::Command::new(program.as_ref());
    cmd.current_dir(&executor.current_directory());
    for a in args {
        match a {
            RainValue::String(a) => cmd.arg(a.as_ref()),
            RainValue::Path(p) => cmd.arg(p.as_ref()),
            _ => {
                return Err(RainError::new(
                    ExecError::UnexpectedType {
                        expected: &[RainType::String],
                        actual: a.as_type(),
                    },
                    fn_call.unwrap().span(),
                )
                .into());
            }
        };
    }
    let status = cmd.status().unwrap();

    let out = Record::new([(String::from("success"), RainValue::Bool(status.success()))]);
    Ok(RainValue::Record(out))
}

fn execute_download(
    _executor: &mut Executor,
    _args: &[RainValue],
    _fn_call: Option<&FnCall<'_>>,
) -> Result<RainValue, ExecCF> {
    todo!()
}

fn execute_path(
    _executor: &mut Executor,
    args: &[RainValue],
    fn_call: Option<&FnCall<'_>>,
) -> Result<RainValue, ExecCF> {
    let [a] = args else {
        return Err(RainError::new(
            ExecError::IncorrectArgCount {
                expected: 1,
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
    Ok(RainValue::Path(Rc::new(PathBuf::from_str(s).unwrap())))
}
