use exec::{
    execution::Execution,
    executor::{BaseExecutor, Executor, ScriptExecutor},
    ExecCF,
};

pub mod ast;
pub mod error;
pub mod exec;
pub mod source;
pub mod span;
pub mod tokens;
mod utils;

pub fn run(source: source::Source, e: &mut BaseExecutor) -> Result<(), Box<error::ResolvedError>> {
    match run_inner(&source, e) {
        Ok(()) => Ok(()),
        Err(ExecCF::Return(_)) => todo!(),
        Err(ExecCF::RuntimeError(_)) => todo!(),
        Err(ExecCF::RainError(err)) => Err(Box::new(err.resolve(source))),
        Err(ExecCF::ResolvedRainError(err)) => Err(err),
    }
}

fn run_inner(source: &source::Source, e: &mut BaseExecutor) -> Result<(), ExecCF> {
    let mut token_stream = tokens::peek_stream::PeekTokenStream::new(&source.source);
    let script: ast::script::Script = ast::script::Script::parse_stream(&mut token_stream)?;
    let mut script_executor = ScriptExecutor::new(source.path.directory().unwrap(), source.clone());
    let mut executor = Executor::new(e, &mut script_executor);
    script.statements.execute(&mut executor)?;
    Ok(())
}
