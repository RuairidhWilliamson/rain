use std::{
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

use clap::Args;
use owo_colors::OwoColorize;
use rain_lang::{
    ast::{
        declaration::{Declaration, InnerDeclaration},
        script::Script,
    },
    exec::{
        types::{function::Function, RainValue},
        ExecCF, ExecuteOptions,
    },
    executor::{builder::ExecutorBuilder, script::ScriptExecutor, Executor},
    manifest::Manifest,
    path::Workspace,
    source::Source,
    tokens::peek_stream::PeekTokenStream,
};

use crate::error_display::ErrorDisplay;

#[derive(Args)]
pub struct RunCommand {
    /// Target to run
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
        let manifest = Self::resolve_manifest(workspace);
        let path = self.path.as_deref().unwrap_or_else(|| Path::new("."));
        let source = match Source::new(&workspace.new_path(path)) {
            Ok(source) => source,
            Err(err) => {
                eprintln!("Could not open file at path {:?}: {err:#}", path);
                return ExitCode::FAILURE;
            }
        };
        match self.run_inner(&source, &manifest, workspace) {
            Ok(()) => ExitCode::SUCCESS,
            Err(ExecCF::Return(_, _)) => unreachable!("return control flow is caught earlier"),
            Err(ExecCF::RuntimeError(err)) => err.display(),
            Err(ExecCF::RainError(err)) => err.resolve(source).display(),
            Err(ExecCF::ResolvedRainError(err)) => err.display(),
        }
    }

    fn resolve_manifest(workspace: &Workspace) -> Manifest {
        Manifest::load(&workspace.resolve().join("rain.toml"))
    }

    fn run_inner(
        self,
        source: &Source,
        manifest: &Manifest,
        workspace: &Workspace,
    ) -> Result<(), ExecCF> {
        let mut token_stream = PeekTokenStream::new(&source.source);
        let script = Script::parse_stream(&mut token_stream)?;
        let script_executor = ScriptExecutor::new(script, source.clone())?;
        if let Some(target) = &self.target {
            let Some(t) = script_executor.get(target).cloned() else {
                eprintln!("{}", "Unknown target, specify a target:".red());
                self.print_targets(&script_executor);
                return Ok(());
            };
            let Declaration {
                inner: InnerDeclaration::Function(func),
                ..
            } = t
            else {
                panic!("not a function");
            };
            let options = ExecuteOptions::default();
            let mut base_executor = ExecutorBuilder {
                dependencies: manifest.dependencies.clone(),
                stdlib: Some(crate::stdlib::new_stdlib()),
                options,
                ..Default::default()
            }
            .build(workspace.clone());
            let mut executor = Executor::new(&mut base_executor, &script_executor);
            let output =
                Function::new(script_executor.clone(), func).call(&mut executor, &[], None)?;
            if self.show_leaves {
                eprintln!("{:?}", executor.leaves);
            }
            if !matches!(output, RainValue::Void) {
                println!("{output:?}");
            }
            if self.execute_output {
                self.execute_output(output);
            }
        } else {
            eprintln!("{}", "Specify a target:".red());
            self.print_targets(&script_executor)
        }
        Ok(())
    }

    fn print_targets(&self, script: &ScriptExecutor) {
        for (k, d) in script {
            if d.visibility.is_none() {
                continue;
            }
            let Declaration {
                inner: InnerDeclaration::Function(func),
                ..
            } = d
            else {
                continue;
            };
            if !func.args.is_empty() {
                continue;
            }
            if let Some(comment) = &d.comment {
                eprintln!("{}: {}", k.magenta(), comment.blue());
            } else {
                eprintln!("{}", k.magenta());
            }
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
