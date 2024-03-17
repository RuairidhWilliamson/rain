use std::path::{Path, PathBuf};
use std::rc::Rc;

use rain_lang::ast::fn_call::FnCall;
use rain_lang::error::RainError;
use rain_lang::exec::types::RainType;
use rain_lang::exec::ExecError;
use rain_lang::exec::{
    types::{function::Function, record::Record, RainValue},
    Executor,
};

pub fn std_lib() -> Record {
    let mut std_lib = Record::default();
    std_lib.insert(
        String::from("run"),
        RainValue::Function(Function::new_external(execute_run)),
    );
    std_lib.insert(String::from("escape"), RainValue::Record(std_escape_lib()));
    std_lib
}

fn execute_run(
    _executor: &mut Executor,
    args: &[RainValue],
    fn_call: &FnCall<'_>,
) -> Result<RainValue, RainError> {
    let Some((program, args)) = args.split_first() else {
        return Err(RainError::new(
            ExecError::IncorrectArgCount {
                expected: 1,
                actual: 0,
            },
            fn_call.span,
        ));
    };
    let RainValue::Path(program) = program else {
        return Err(RainError::new(
            ExecError::UnexpectedType {
                expected: &[RainType::Path],
                actual: program.as_type(),
            },
            fn_call.span,
        ));
    };
    let args = args
        .iter()
        .map(|a| {
            let RainValue::String(a) = a else {
                return Err(RainError::new(
                    ExecError::UnexpectedType {
                        expected: &[RainType::String],
                        actual: a.as_type(),
                    },
                    fn_call.span,
                ));
            };
            Ok(a.as_ref())
        })
        .collect::<Result<Vec<_>, RainError>>()?;
    let status = std::process::Command::new(program.as_ref())
        .args(args)
        .status()
        .unwrap();
    assert!(status.success());
    Ok(RainValue::Unit)
}

pub fn std_escape_lib() -> Record {
    let mut std_escape_lib = Record::default();
    std_escape_lib.insert(
        String::from("bin"),
        RainValue::Function(Function::new_external(execute_bin)),
    );
    std_escape_lib
}

fn execute_bin(
    _executor: &mut Executor,
    args: &[RainValue],
    fn_call: &FnCall<'_>,
) -> Result<RainValue, RainError> {
    let [arg] = args else {
        return Err(RainError::new(
            ExecError::IncorrectArgCount {
                expected: 1,
                actual: args.len(),
            },
            fn_call.span,
        ));
    };
    let RainValue::String(name) = arg else {
        return Err(RainError::new(
            ExecError::UnexpectedType {
                expected: &[RainType::String],
                actual: arg.as_type(),
            },
            fn_call.span,
        ));
    };
    let path = find_bin_in_path(name).unwrap();
    Ok(RainValue::Path(Rc::new(path)))
}

fn find_bin_in_path(name: &str) -> Option<PathBuf> {
    std::env::var("PATH")
        .unwrap()
        .split(':')
        .find_map(|p| find_bin_in_dir(Path::new(p), name))
}

fn find_bin_in_dir(dir: &Path, name: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.find_map(|e| {
        let p = e.ok()?;
        if p.file_name().to_str()? == name {
            Some(p.path())
        } else {
            None
        }
    })
}
