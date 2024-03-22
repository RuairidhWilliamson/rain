use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str::FromStr;

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
    std_lib.insert(
        String::from("download"),
        RainValue::Function(Function::new_external(execute_download)),
    );
    std_lib.insert(
        String::from("path"),
        RainValue::Function(Function::new_external(execute_path)),
    );
    std_lib.insert(String::from("escape"), RainValue::Record(std_escape_lib()));
    std_lib
}

fn execute_run(
    executor: &mut Executor,
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
    let mut cmd = std::process::Command::new(program.as_ref());
    cmd.current_dir(&executor.current_directory);
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
                    fn_call.span,
                ));
            }
        };
    }
    let status = cmd.status().unwrap();
    assert!(status.success());
    Ok(RainValue::Unit)
}

fn execute_download(
    _executor: &mut Executor,
    _args: &[RainValue],
    _fn_call: &FnCall<'_>,
) -> Result<RainValue, RainError> {
    todo!()
}

fn execute_path(
    _executor: &mut Executor,
    args: &[RainValue],
    fn_call: &FnCall<'_>,
) -> Result<RainValue, RainError> {
    let [a] = args else {
        return Err(RainError::new(
            ExecError::IncorrectArgCount {
                expected: 1,
                actual: args.len(),
            },
            fn_call.span,
        ));
    };
    let RainValue::String(s) = a else {
        return Err(RainError::new(
            ExecError::UnexpectedType {
                expected: &[RainType::String],
                actual: a.as_type(),
            },
            fn_call.span,
        ));
    };
    Ok(RainValue::Path(Rc::new(PathBuf::from_str(&s).unwrap())))
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
