use std::path::PathBuf;

use exec::ExecutorBuilder;

pub mod ast;
pub mod error;
pub mod exec;
pub mod span;
pub mod tokens;
mod utils;

#[derive(Debug, Clone)]
pub struct Source {
    pub path: PathBuf,
    pub source: String,
}

pub fn run(source: &Source, e: ExecutorBuilder) -> Result<(), error::ResolvedError> {
    run_inner(source, e).map_err(|err| err.resolve(source))
}

fn run_inner(source: &Source, builder: ExecutorBuilder) -> Result<(), error::RainError> {
    // TODO: We should properly track the lifetime of the source code
    let s = source.source.to_owned().leak();
    let mut token_stream = tokens::peek_stream::PeekTokenStream::new(s);
    let script: ast::script::Script<'static> =
        ast::script::Script::parse_stream(&mut token_stream)?;
    let mut executor = builder.build();
    exec::Executable::execute(&script, &mut executor)?;
    Ok(())
}
