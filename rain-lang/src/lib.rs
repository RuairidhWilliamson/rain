use std::path::PathBuf;

pub mod ast;
pub mod error;
pub mod exec;
pub mod span;
pub mod tokens;
mod utils;

#[derive(Debug)]
pub struct Source {
    pub path: PathBuf,
    pub source: String,
}

pub fn run(source: &Source) -> Result<(), error::ResolvedError> {
    run_inner(source).map_err(|err| err.resolve(source))
}

fn run_inner(source: &Source) -> Result<(), error::RainError> {
    // TODO: We should properly track the lifetime of the source code
    let s = source.source.to_owned().leak();
    let mut token_stream = tokens::peek_stream::PeekTokenStream::new(s);
    let script = ast::script::Script::parse_stream(&mut token_stream)?;
    exec::execute(&script, &source.path, None, Default::default())?;
    Ok(())
}
