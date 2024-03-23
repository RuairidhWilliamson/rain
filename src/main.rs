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
    let source = read_src(cli.script.as_deref().unwrap_or(Path::new(".")))?;
    if let Err(err) = main_inner(&source, &cli) {
        let err = err.resolve(&source);
        eprintln!("{err:#}");
        exit(1)
    }
    Ok(())
}

fn read_src(path: &Path) -> color_eyre::Result<Source> {
    let f = std::fs::File::open(path)?;
    let metadata = f.metadata()?;
    if !metadata.is_dir() {
        let source = std::io::read_to_string(f)?;
        return Ok(Source {
            path: path.to_path_buf(),
            source,
        });
    }
    let new_path = path.join("main.rain");
    tracing::debug!("{path:?} is a directory using {new_path:?}");
    let source = std::fs::read_to_string(&new_path)?;
    Ok(Source {
        path: new_path,
        source,
    })
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
        let mut executor = rain_lang::exec::ExecutorBuilder {
            current_directory: source.path.parent().unwrap().to_path_buf(),
            std_lib: Some(stdlib::std_lib()),
            options,
            ..Default::default()
        }
        .build();
        rain_lang::exec::Executable::execute(&script, &mut executor)?;
    }
    Ok(())
}
