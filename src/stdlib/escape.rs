use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

use rain_lang::{
    ast::{function_call::FnCall, Ast},
    error::RainError,
    exec::{
        executor::Executor,
        external::extract_arg,
        types::{
            function::{ExternalFn, Function, FunctionArguments},
            record::Record,
            RainType, RainValue,
        },
        ExecCF, ExecError,
    },
    leaf::Leaf,
    path::RainPath,
    utils::copy_create_dirs,
};

pub fn std_escape_lib() -> Record {
    Record::new([
        (
            String::from("bin"),
            RainValue::Function(Function::new_external(EscapeBin)),
        ),
        (
            String::from("run"),
            RainValue::Function(Function::new_external(EscapeRun)),
        ),
    ])
}

struct EscapeBin;

impl ExternalFn for EscapeBin {
    fn call(
        &self,
        _executor: &mut Executor,
        args: &FunctionArguments,
        fn_call: Option<&FnCall>,
    ) -> Result<RainValue, ExecCF> {
        let [arg] = args else {
            return Err(RainError::new(
                ExecError::IncorrectArgCount {
                    expected: (1..=1).into(),
                    actual: args.len(),
                },
                fn_call.unwrap().span(),
            )
            .into());
        };
        let (_, RainValue::String(name)) = arg else {
            return Err(RainError::new(
                ExecError::UnexpectedType {
                    expected: &[RainType::String],
                    actual: arg.1.as_type(),
                },
                fn_call.unwrap().span(),
            )
            .into());
        };
        let path = RainPath::escaped(find_bin_in_path(name).unwrap());
        Ok(RainValue::File(Rc::new(path)))
    }
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

struct EscapeRun;

impl ExternalFn for EscapeRun {
    fn call(
        &self,
        executor: &mut Executor,
        args: &FunctionArguments,
        fn_call: Option<&FnCall>,
    ) -> Result<RainValue, ExecCF> {
        let program = extract_arg(args, "program", None, fn_call)?;
        let RainValue::File(program) = program else {
            return Err(RainError::new(
                ExecError::UnexpectedType {
                    expected: &[RainType::File],
                    actual: program.as_type(),
                },
                fn_call.unwrap().span(),
            )
            .into());
        };
        let program_args = extract_arg(args, "args", None, fn_call)?;
        let RainValue::List(program_args) = program_args else {
            return Err(RainError::new(
                ExecError::UnexpectedType {
                    expected: &[RainType::List],
                    actual: program_args.as_type(),
                },
                fn_call.unwrap().span(),
            )
            .into());
        };
        executor.leaves.insert(Leaf::File(program.as_ref().clone()));
        let mut cmd = std::process::Command::new(program.resolve());
        cmd.current_dir(&executor.base_executor.root_workspace.resolve());
        for a in program_args.iter() {
            match a {
                RainValue::String(a) => cmd.arg(a.as_ref()),
                RainValue::Path(p) => cmd.arg(p.as_ref()),
                RainValue::File(f) => {
                    executor.leaves.insert(Leaf::File(f.as_ref().clone()));
                    let path = f.resolve();
                    let exec_path = executor
                        .base_executor
                        .root_workspace
                        .new_path(f.workspace_relative_directory());
                    copy_create_dirs(&path, &exec_path.resolve()).unwrap();
                    cmd.arg(f.workspace_relative_directory())
                }
                _ => {
                    return Err(RainError::new(
                        ExecError::UnexpectedType {
                            expected: &[RainType::String, RainType::Path, RainType::File],
                            actual: a.as_type(),
                        },
                        fn_call.unwrap().span(),
                    )
                    .into());
                }
            };
        }
        tracing::info!("std.escape.run {cmd:?}");
        let output = cmd.output().unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        let exit_code = output
            .status
            .code()
            .map_or_else(|| RainValue::Void, |code| RainValue::Int(code as isize));
        let out = Record::new([
            (String::from("exit_code"), exit_code),
            (
                String::from("success"),
                RainValue::Bool(output.status.success()),
            ),
            (String::from("stdout"), stdout.into()),
            (String::from("stderr"), stderr.into()),
        ]);
        Ok(RainValue::Record(out))
    }
}
