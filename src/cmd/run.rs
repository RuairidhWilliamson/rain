use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::Args;
use rain_lang::{
    ast::script::Script,
    exec::{
        executor::{Executor, ScriptExecutor},
        types::RainValue,
        ExecCF,
    },
    source::Source,
};

use crate::error_display::ErrorDisplay;

#[derive(Args)]
pub struct RunCommand {
    target: Option<String>,

    #[arg(long)]
    path: Option<PathBuf>,

    #[arg(long)]
    execute_output: bool,
}

impl RunCommand {
    pub fn run(self) -> ExitCode {
        let path = self.path.as_deref().unwrap_or(Path::new("."));
        let source = match Source::new(path) {
            Ok(source) => source,
            Err(err) => {
                eprintln!("Could not open file at path {:?}: {err:#}", path);
                return ExitCode::FAILURE;
            }
        };
        match self.run_inner(&source) {
            Ok(()) => ExitCode::SUCCESS,
            Err(ExecCF::Return(_)) => unreachable!("return control flow is caught earlier"),
            Err(ExecCF::RuntimeError(err)) => err.display(),
            Err(ExecCF::RainError(err)) => err.resolve(&source).display(),
        }
    }

    fn run_inner(self, source: &Source) -> Result<(), ExecCF> {
        // TODO: We should properly track the lifetime of the source code
        let s = Into::<String>::into(&source.source).leak();
        let mut token_stream = rain_lang::tokens::peek_stream::PeekTokenStream::new(s);

        let script = Script::parse_stream(&mut token_stream)?;
        let options = rain_lang::exec::ExecuteOptions { sealed: false };
        let mut base_executor = rain_lang::exec::executor::ExecutorBuilder {
            current_directory: source.path.directory().unwrap().to_path_buf(),
            std_lib: Some(crate::stdlib::std_lib()),
            options,
            ..Default::default()
        }
        .build();
        let mut script_executor = ScriptExecutor::new(&base_executor);
        let mut executor = Executor::new(&mut base_executor, &mut script_executor);
        rain_lang::exec::execution::Execution::execute(&script, &mut executor)?;
        if let Some(target) = &self.target {
            let t = script_executor.global_record.get(target).unwrap();
            let RainValue::Function(func) = t else {
                panic!("not a function");
            };
            let mut executor = Executor::new(&mut base_executor, &mut script_executor);
            let output = func.call(&mut executor, &[], None)?;
            if self.execute_output {
                println!("{output:?}");
                todo!()
            }
        } else {
            eprintln!("Specify a target: {}", script_executor.global_record);
        }
        Ok(())
    }
}
