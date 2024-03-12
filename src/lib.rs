use std::path::Path;

pub mod ast;
pub mod error;
pub mod exec;
pub mod span;
pub mod tokens;

pub fn run<'a>(source_path: &'a Path, source: &'a str) -> Result<(), error::ResolvedError<'a>> {
    run_inner(source).map_err(|err| err.resolve(source_path, source))
}

fn run_inner(source: impl Into<String>) -> Result<(), error::RainError> {
    // TODO: We should properly track the lifetime of the source code
    let source = Into::<String>::into(source).leak();
    let mut token_stream = tokens::peek_stream::PeekTokenStream::new(source);
    let script = ast::script::Script::parse_stream(&mut token_stream)?;
    exec::execute(&script, Default::default())?;
    Ok(())
}
