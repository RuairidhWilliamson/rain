use std::{
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

use clap::Args;
use rain_lang::{
    ast::{declaration::Declaration, script::Script},
    exec::{
        executor::{Executor, ExecutorBuilder},
        script::ScriptExecutor,
        types::{function::Function, RainValue},
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
        let script_executor = ScriptExecutor::new(script, source.clone())?;
        if let Some(target) = &self.target {
            let Some(t) = script_executor.get(target).cloned() else {
                eprintln!("Unknown target, specify a target:",);
                self.print_targets(&script_executor);
                return Ok(());
            };
            let Declaration::FnDeclare(func) = t else {
                panic!("not a function");
            };
            let options = ExecuteOptions::default();
            let mut base_executor = ExecutorBuilder {
                stdlib: Some(crate::stdlib::new_stdlib()),
                options,
                ..Default::default()
            }
            .build(workspace.clone());
            let mut executor = Executor::new(&mut base_executor, &script_executor);
            let output = Function::new(source.clone(), func).call(&mut executor, &[], None)?;
            if self.show_leaves {
                eprintln!("{:?}", executor.leaves);
            }
            println!("{output:?}");
            if self.execute_output {
                self.execute_output(output);
            }
        } else {
            eprintln!("Specify a target:");
            self.print_targets(&script_executor)
        }
        Ok(())
    }

    fn print_targets(&self, script: &ScriptExecutor) {
        let mut records: Vec<(&String, &Declaration)> = script.into_iter().collect();
        records.sort_unstable_by(|(a, _), (b, _)| a.cmp(b));
        for (k, d) in records {
            let Declaration::FnDeclare(d) = d else {
                continue;
            };
            if d.visibility.is_none() {
                continue;
            }
            eprintln!("{k}");
        }
    }

    fn execute_output(&self, output: RainValue) -> ExitCode {
        let RainValue::File(p) = output else {
            eprintln!(
                "Output is the wrong type expected file, got {:?}",
                output.as_type()
            );
            return ExitCode::FAILURE;
        };
        eprintln!("=== Executing output ===");
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
