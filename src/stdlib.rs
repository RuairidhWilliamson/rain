mod escape;

use std::rc::Rc;

use rain_lang::ast::function_call::FnCall;
use rain_lang::ast::Ast;
use rain_lang::config::{global_config, Config};
use rain_lang::error::RainError;
use rain_lang::exec::executor::Executor;
use rain_lang::exec::external::extract_arg;
use rain_lang::exec::types::function::{ExternalFn, FunctionArguments};
use rain_lang::exec::types::RainType;
use rain_lang::exec::types::{function::Function, record::Record, RainValue};
use rain_lang::exec::{ExecCF, ExecError};
use rain_lang::leaf::Leaf;
use rain_lang::path::RainPath;
use rain_lang::utils::copy_create_dirs;

pub fn new_stdlib() -> Record {
    let config = global_config();
    Record::new([
        (
            String::from("run"),
            RainValue::Function(Function::new_external(StdRun { config })),
        ),
        (
            String::from("generated"),
            RainValue::Function(Function::new_external(StdGenerated { config })),
        ),
        (
            String::from("download"),
            RainValue::Function(Function::new_external(StdDownload { config })),
        ),
        (
            String::from("escape"),
            RainValue::Record(escape::std_escape_lib()),
        ),
    ])
}

struct StdRun {
    config: &'static rain_lang::config::Config,
}

impl ExternalFn for StdRun {
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
        let extra_files = extract_arg(args, "extras", None, fn_call)?;
        let RainValue::List(extra_files) = extra_files else {
            return Err(RainError::new(
                ExecError::UnexpectedType {
                    expected: &[RainType::List],
                    actual: extra_files.as_type(),
                },
                fn_call.unwrap().span(),
            )
            .into());
        };
        tracing::info!("Run {program_args:?}");

        let id: String = uuid::Uuid::new_v4().to_string();
        let exec_directory = self.config.exec_directory().join(&id);
        std::fs::create_dir_all(&exec_directory).unwrap();

        executor.leaves.insert(Leaf::File(program.as_ref().clone()));
        let mut cmd = std::process::Command::new(program.resolve());
        cmd.current_dir(&exec_directory);
        cmd.env_clear();
        for a in program_args.iter() {
            match a {
                RainValue::String(a) => cmd.arg(a.as_ref()),
                RainValue::Path(p) => cmd.arg(p.as_ref()),
                RainValue::File(f) => {
                    executor.leaves.insert(Leaf::File(f.as_ref().clone()));
                    let path = &f.resolve();
                    let workspace_relative_path = f.workspace_relative_directory();
                    let exec_path = exec_directory.join(workspace_relative_path);
                    copy_create_dirs(path, &exec_path).unwrap();

                    cmd.arg(workspace_relative_path)
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
        for a in extra_files.iter() {
            match a {
                RainValue::File(f) => {
                    executor.leaves.insert(Leaf::File(f.as_ref().clone()));
                    let path = &f.resolve();
                    let workspace_relative_path = f.workspace_relative_directory();
                    let exec_path = exec_directory.join(workspace_relative_path);
                    copy_create_dirs(path, &exec_path).unwrap();
                }
                _ => {
                    return Err(RainError::new(
                        ExecError::UnexpectedType {
                            expected: &[RainType::File],
                            actual: a.as_type(),
                        },
                        fn_call.unwrap().span(),
                    )
                    .into())
                }
            }
        }
        tracing::info!("std.run {cmd:?}");
        let output = cmd.output().unwrap();

        let out = Record::new([
            (String::from("id"), RainValue::String(id.as_str().into())),
            (
                String::from("success"),
                RainValue::Bool(output.status.success()),
            ),
        ]);
        Ok(RainValue::Record(out))
    }
}

struct StdGenerated {
    config: &'static Config,
}

impl ExternalFn for StdGenerated {
    fn call(
        &self,
        _executor: &mut Executor,
        args: &FunctionArguments,
        fn_call: Option<&FnCall>,
    ) -> Result<RainValue, ExecCF> {
        let [run, path] = args else {
            return Err(RainError::new(
                ExecError::IncorrectArgCount {
                    expected: (2..=2).into(),
                    actual: args.len(),
                },
                fn_call.unwrap().span(),
            )
            .into());
        };

        let (_, RainValue::Record(run)) = run else {
            return Err(RainError::new(
                ExecError::UnexpectedType {
                    expected: &[RainType::Record],
                    actual: run.1.as_type(),
                },
                fn_call.unwrap().span(),
            )
            .into());
        };
        let (_, RainValue::Path(path)) = path else {
            return Err(RainError::new(
                ExecError::UnexpectedType {
                    expected: &[RainType::Path],
                    actual: path.1.as_type(),
                },
                fn_call.unwrap().span(),
            )
            .into());
        };

        let Some(RainValue::String(id)) = run.get("id") else {
            panic!("id not set");
        };
        let exec_directory = self.config.exec_directory().join(id.as_ref());
        let p = exec_directory.join(path.as_ref());
        let new_path = RainPath::generated(uuid::Uuid::new_v4(), path.as_ref().into());
        copy_create_dirs(&p, &new_path.resolve()).unwrap();
        Ok(RainValue::File(Rc::new(new_path)))
    }
}

struct StdDownload {
    #[allow(dead_code)]
    config: &'static Config,
}

impl ExternalFn for StdDownload {
    #[allow(unused)]
    fn call(
        &self,
        executor: &mut Executor,
        args: &FunctionArguments,
        call: Option<&FnCall>,
    ) -> Result<RainValue, ExecCF> {
        todo!("implement std.download")
    }
}
