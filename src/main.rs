mod stdlib;

use std::{
    path::{Path, PathBuf},
    process::exit,
};

use clap::Parser;
use color_eyre::owo_colors::OwoColorize;
use rain_lang::{
    ast::script::Script,
    error::ResolvedError,
    exec::{
        executor::{Executor, ScriptExecutor},
        types::RainValue,
        ExecCF,
    },
    source::Source,
};

#[derive(Parser)]
struct Cli {
    target: Option<String>,

    #[arg(long)]
    path: Option<PathBuf>,

    #[arg(long)]
    no_exec: bool,

    #[arg(long)]
    print_ast: bool,

    #[arg(long)]
    sealed: bool,
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let source = Source::new(cli.path.as_deref().unwrap_or(Path::new(".")))?;
    match main_inner(&source, &cli) {
        Ok(()) => Ok(()),
        Err(ExecCF::Return(_)) => todo!(),
        Err(ExecCF::RuntimeError(err)) => {
            eprintln!("{}: {}", "runtime error".bold().red(), err.msg.bold());
            exit(1)
        }
        Err(ExecCF::RainError(err)) => {
            let ResolvedError { source, err } = err.resolve(&source);
            let extract = err.span.extract(&source.source);
            let lineno = err.span.start.line + 1;
            eprintln!("{}: {}", "error".bold().red(), err.kind.bold());
            eprintln!("\t{}:{}", source.path.yellow(), lineno.yellow());
            eprintln!("\t{}", extract.line);
            eprintln!("\t{}", extract.under_arrows().red());
            exit(1)
        }
    }
}

fn main_inner(source: &Source, cli: &Cli) -> Result<(), ExecCF> {
    // TODO: We should properly track the lifetime of the source code
    let s = Into::<String>::into(&source.source).leak();
    let mut token_stream = rain_lang::tokens::peek_stream::PeekTokenStream::new(s);

    let script = Script::parse_stream(&mut token_stream)?;
    if cli.print_ast {
        println!("{script:#?}");
    }

    if !cli.no_exec {
        let options = rain_lang::exec::ExecuteOptions { sealed: cli.sealed };
        let mut base_executor = rain_lang::exec::executor::ExecutorBuilder {
            current_directory: source.path.directory().unwrap().to_path_buf(),
            std_lib: Some(stdlib::std_lib()),
            options,
            ..Default::default()
        }
        .build();
        let mut script_executor = ScriptExecutor::new(&base_executor);
        let mut executor = Executor::new(&mut base_executor, &mut script_executor);
        rain_lang::exec::execution::Execution::execute(&script, &mut executor)?;
        if let Some(target) = &cli.target {
            let t = script_executor.global_record.get(target).unwrap();
            let RainValue::Function(func) = t else {
                panic!("not a function");
            };
            let mut executor = Executor::new(&mut base_executor, &mut script_executor);
            func.call(&mut executor, &[], None)?;
        } else {
            eprintln!("Specify a target: {}", script_executor.global_record);
        }
    }
    Ok(())
}
