use std::{
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

use clap::Args;
use rain_lang::{
    ast::script::Script,
    exec::{
        execution::Execution,
        executor::{Executor, ExecutorBuilder, ScriptExecutor},
        types::RainValue,
        ExecCF, ExecuteOptions,
    },
    path::Workspace,
    source::Source,
    tokens::peek_stream::PeekTokenStream,
};

use crate::error_display::ErrorDisplay;

#[derive(Args)]
pub struct RunCommand {
    target: Option<String>,

    #[arg(long)]
    path: Option<PathBuf>,

    #[arg(long)]
    show_leaves: bool,

    #[arg(long)]
    execute_output: bool,
}

impl RunCommand {
    pub fn run(self, workspace: &Workspace) -> ExitCode {
        let path = self.path.as_deref().unwrap_or_else(|| Path::new("."));
        let source = match Source::new(&workspace.new_path(path)) {
            Ok(source) => source,
            Err(err) => {
                eprintln!("Could not open file at path {:?}: {err:#}", path);
                return ExitCode::FAILURE;
            }
        };
        match self.run_inner(&source, workspace) {
            Ok(()) => ExitCode::SUCCESS,
            Err(ExecCF::Return(_, _)) => unreachable!("return control flow is caught earlier"),
            Err(ExecCF::RuntimeError(err)) => err.display(),
            Err(ExecCF::RainError(err)) => err.resolve(source).display(),
            Err(ExecCF::ResolvedRainError(err)) => err.display(),
        }
    }

    fn run_inner(self, source: &Source, workspace: &Workspace) -> Result<(), ExecCF> {
        let mut token_stream = PeekTokenStream::new(&source.source);
        let script = Script::parse_stream(&mut token_stream)?;
        let options = ExecuteOptions::default();
        let mut base_executor = ExecutorBuilder {
            stdlib: Some(crate::stdlib::new_stdlib()),
            options,
            ..Default::default()
        }
        .build(workspace.clone());
        let mut script_executor = ScriptExecutor::new(source.clone());
        let mut executor = Executor::new(&mut base_executor, &mut script_executor);
        Execution::execute(&script, &mut executor)?;
        if let Some(target) = &self.target {
            let Some(t) = script_executor.global_record.get(target) else {
                eprintln!(
                    "Unknown target, choose one of: {}",
                    script_executor.global_record
                );
                return Ok(());
            };
            let RainValue::Function(func) = t else {
                panic!("not a function");
            };
            let mut executor = Executor::new(&mut base_executor, &mut script_executor);
            let output = func.call(&mut executor, &[], None)?;
            if self.show_leaves {
                eprintln!("{:?}", executor.leaves);
            }
            println!("{output:?}");
            if self.execute_output {
                self.execute_output(output);
            }
        } else {
            eprintln!("Specify a target: {}", script_executor.global_record);
        }
        Ok(())
    }

    fn execute_output(&self, output: RainValue) -> ExitCode {
        let RainValue::File(p) = output else {
            eprintln!(
                "Output is the wrong type expected file, got {:?}",
                output.as_type()
            );
            return ExitCode::FAILURE;
        };
        if Command::new(p.resolve())
            .status()
            .expect("execute output")
            .success()
        {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        }
    }
}
