mod escape;

use std::path::Path;

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

        let mut program_path = program.absolute();
        program_path = program_path
            .strip_prefix(executor.current_directory())
            .map(|p| p.to_path_buf())
            .unwrap_or(program_path);
        if program_path.is_relative() {
            program_path = Path::new(".").join(program_path);
        }

        let mut cmd = std::process::Command::new(&program_path);
        cmd.current_dir(executor.current_directory());
        for a in args {
            match a {
                RainValue::String(a) => cmd.arg(a.as_ref()),
                RainValue::Path(p) => cmd.arg(
                    p.absolute()
                        .strip_prefix(executor.current_directory())
                        .unwrap(),
                ),
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

        let out = Record::new([(String::from("success"), RainValue::Bool(status.success()))]);
        Ok(RainValue::Record(out))
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
