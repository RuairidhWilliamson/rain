mod stdlib;

use std::{
    path::{Path, PathBuf},
    process::exit,
};

use clap::Parser;
use rain_lang::{ast::script::Script, error::RainError, Source};

#[derive(Parser)]
struct Cli {
    script: Option<PathBuf>,

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
    let source = Source::new(cli.script.as_deref().unwrap_or(Path::new(".")))?;
    if let Err(err) = main_inner(&source, &cli) {
        let err = err.resolve(&source);
        eprintln!("{err:#}");
        exit(1)
    }
    Ok(())
}

fn main_inner(source: &Source, cli: &Cli) -> Result<(), RainError> {
    // TODO: We should properly track the lifetime of the source code
    let s = Into::<String>::into(&source.source).leak();
    let mut token_stream = rain_lang::tokens::peek_stream::PeekTokenStream::new(s);

    let script = Script::parse_stream(&mut token_stream)?;
    if cli.print_ast {
        println!("{script:#?}");
    }

    if !cli.no_exec {
        let options = rain_lang::exec::ExecuteOptions { sealed: cli.sealed };
        let mut global_executor = rain_lang::exec::executor::GlobalExecutorBuilder {
            current_directory: source.path.directory().unwrap().to_path_buf(),
            std_lib: Some(stdlib::std_lib()),
            options,
            ..Default::default()
        }
        .build();
        let mut executor = rain_lang::exec::executor::Executor::new(&mut global_executor);
        rain_lang::exec::executable::Executable::execute(&script, &mut executor)?;
    }
    Ok(())
}
