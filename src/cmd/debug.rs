use std::{path::PathBuf, process::ExitCode};

use clap::Subcommand;
use rain_lang::{ast::script::Script, source::Source};

use crate::error_display::ErrorDisplay;

#[derive(Subcommand)]
pub enum DebugCommand {
    /// Parses and prints the AST for a rain script
    PrintAst { script: PathBuf },
}

impl DebugCommand {
    pub fn run(self) -> ExitCode {
        match self {
            Self::PrintAst { script } => Self::print_ast(script),
        }
    }

    fn print_ast(path: PathBuf) -> ExitCode {
        let source = match Source::new(&path) {
            Ok(source) => source,
            Err(err) => {
                eprintln!("Could not open file at path {:?}: {err:#}", path);
                return ExitCode::FAILURE;
            }
        };

        let mut token_stream = rain_lang::tokens::peek_stream::PeekTokenStream::new(&source.source);

        let script = match Script::parse_stream(&mut token_stream) {
            Ok(script) => script,
            Err(err) => return err.resolve(source).display(),
        };
        println!("{script:#?}");
        ExitCode::SUCCESS
    }
}
