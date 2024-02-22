use std::path::Path;

pub mod ast;
pub mod error;
pub mod exec;
pub mod span;
pub mod tokens;

pub fn run<'a>(source_path: &'a Path, source: &'a str) -> Result<(), error::ResolvedError<'a>> {
    run_inner(source).map_err(|err| err.resolve(source_path, source))
}

fn run_inner(source: &str) -> Result<(), error::RainError> {
    let tokens = tokens::TokenStream::new(source).parse_collect()?;
    let script = ast::Script::parse(&tokens)?;
    exec::execute(&script, Default::default())?;
    Ok(())
}
