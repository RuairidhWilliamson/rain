mod escape;

use std::rc::Rc;

use rain_lang::ast::function_call::FnCall;
use rain_lang::ast::Ast;
use rain_lang::error::RainError;
use rain_lang::exec::executor::Executor;
use rain_lang::exec::types::RainType;
use rain_lang::exec::types::{function::Function, record::Record, RainValue};
use rain_lang::exec::{ExecCF, ExecError};

pub fn new_stdlib(config: &'static crate::config::Config) -> Record {
    let stdlib = Box::leak(Box::new(Stdlib { config }));
    Record::new([
        (String::from("run"), define_function(stdlib, Stdlib::run)),
        (
            String::from("generated"),
            define_function(stdlib, Stdlib::generated),
        ),
        (
            String::from("download"),
            define_function(stdlib, Stdlib::download),
        ),
        (
            String::from("escape"),
            RainValue::Record(escape::std_escape_lib()),
        ),
    ])
}

fn define_function(
    stdlib: &'static Stdlib,
    func: impl Fn(
            &Stdlib,
            &mut Executor<'_>,
            &[RainValue],
            Option<&FnCall<'_>>,
        ) -> Result<RainValue, ExecCF>
        + 'static,
) -> RainValue {
    RainValue::Function(Function::new_external(
        move |executor: &mut Executor<'_>, args: &[RainValue], fn_call: Option<&FnCall<'_>>| {
            func(stdlib, executor, args, fn_call)
        },
    ))
}

struct Stdlib {
    config: &'static crate::config::Config,
}

impl Stdlib {
    fn run(
        &self,
        executor: &mut Executor,
        args: &[RainValue],
        fn_call: Option<&FnCall<'_>>,
    ) -> Result<RainValue, ExecCF> {
        let Some((program, args)) = args.split_first() else {
            return Err(RainError::new(
                ExecError::IncorrectArgCount {
                    expected: (1..).into(),
                    actual: 0,
                },
                fn_call.unwrap().span(),
            )
            .into());
        };
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

        let id: String = uuid::Uuid::new_v4().to_string();
        let exec_directory = self.config.exec_directory().join(&id);
        std::fs::create_dir_all(&exec_directory).unwrap();

        let mut cmd = std::process::Command::new(&program.path);
        cmd.current_dir(&exec_directory);
        for a in args {
            match a {
                RainValue::String(a) => cmd.arg(a.as_ref()),
                RainValue::Path(p) => cmd.arg(p.relative_workspace()),
                RainValue::File(f) => {
                    let path = &f.path;
                    let workspace_relative_path = path
                        .strip_prefix(&executor.base_executor.workspace_directory)
                        .unwrap();
                    let exec_path = exec_directory.join(workspace_relative_path);
                    tracing::info!("Copying {path:?} to {:?}", exec_path);
                    std::fs::create_dir_all(exec_path.parent().unwrap()).unwrap();
                    std::fs::copy(path, &exec_path).unwrap();
                    cmd.arg(workspace_relative_path)
                }
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
        tracing::info!("std.run {cmd:?}");
        let status = cmd.status().unwrap();

        let out = Record::new([
            (String::from("id"), RainValue::String(id.as_str().into())),
            (String::from("success"), RainValue::Bool(status.success())),
        ]);
        Ok(RainValue::Record(out))
    }

    fn generated(
        &self,
        _executor: &mut Executor,
        args: &[RainValue],
        fn_call: Option<&FnCall<'_>>,
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

        let RainValue::Record(run) = run else {
            return Err(RainError::new(
                ExecError::UnexpectedType {
                    expected: &[RainType::Record],
                    actual: run.as_type(),
                },
                fn_call.unwrap().span(),
            )
            .into());
        };
        let RainValue::Path(path) = path else {
            return Err(RainError::new(
                ExecError::UnexpectedType {
                    expected: &[RainType::Path],
                    actual: path.as_type(),
                },
                fn_call.unwrap().span(),
            )
            .into());
        };

        let Some(RainValue::String(id)) = run.get("id") else {
            panic!("id not set");
        };
        let exec_directory = self.config.exec_directory().join(id.as_ref());
        let p = exec_directory.join(path.relative_workspace());
        let new_path = self.config.out_directory().join(path.relative_workspace());
        tracing::info!("output copying {p:?} to {new_path:?}");
        std::fs::create_dir_all(new_path.parent().unwrap()).unwrap();
        std::fs::copy(p, &new_path).unwrap();
        Ok(RainValue::File(Rc::new(
            rain_lang::exec::types::file::File {
                kind: rain_lang::exec::types::file::FileKind::Generated,
                path: new_path,
            },
        )))
    }

    fn download(
        &self,
        _executor: &mut Executor,
        _args: &[RainValue],
        _fn_call: Option<&FnCall<'_>>,
    ) -> Result<RainValue, ExecCF> {
        todo!("implement std.download")
    }
}
