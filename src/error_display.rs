use std::process::ExitCode;

use owo_colors::OwoColorize;
use rain_lang::{error::ResolvedError, exec::RuntimeError};

pub trait ErrorDisplay {
    fn display(self) -> ExitCode;
}

impl ErrorDisplay for ResolvedError<'_> {
    fn display(self) -> ExitCode {
        let ResolvedError { source, err } = self;
        let extract = err.span.extract(&source.source);
        let lineno = err.span.start.line + 1;
        eprintln!("{}: {}", "error".bold().red(), err.kind.bold());
        eprintln!("\t{}:{}", source.path.yellow(), lineno.yellow());
        eprintln!("\t{}", extract.line);
        eprintln!("\t{}", extract.under_arrows().red());
        ExitCode::FAILURE
    }
}

impl ErrorDisplay for RuntimeError {
    fn display(self) -> ExitCode {
        eprintln!("{}: {}", "runtime error".bold().red(), self.msg.bold());
        ExitCode::FAILURE
    }
}
