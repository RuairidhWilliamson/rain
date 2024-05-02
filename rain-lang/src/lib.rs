use exec::{
    execution::Execution,
    executor::{BaseExecutor, Executor, ScriptExecutor},
    ExecCF,
};

pub mod ast;
pub mod cache;
pub mod config;
pub mod error;
pub mod exec;
pub mod leaf;
pub mod path;
pub mod source;
pub mod span;
pub mod tokens;
pub mod utils;

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("runtime error: {0:?}")]
    RuntimeError(exec::RuntimeError),
    #[error("{0}")]
    ResolvedRainError(error::ResolvedError),
}

pub fn run(source: source::Source, e: &mut BaseExecutor) -> Result<(), RunError> {
    match run_inner(&source, e) {
        Ok(()) => Ok(()),
        Err(ExecCF::Return(_, span)) => Err(RunError::ResolvedRainError(
            error::RainError::new(exec::ExecError::ReturnOutsideFunction, span).resolve(source),
        )),
        Err(ExecCF::RuntimeError(err)) => Err(RunError::RuntimeError(err)),
        Err(ExecCF::RainError(err)) => Err(RunError::ResolvedRainError(err.resolve(source))),
        Err(ExecCF::ResolvedRainError(err)) => Err(RunError::ResolvedRainError(*err)),
    }
}

fn run_inner(source: &source::Source, e: &mut BaseExecutor) -> Result<(), ExecCF> {
    let mut token_stream = tokens::peek_stream::PeekTokenStream::new(&source.source);
    let script: ast::script::Script = ast::script::Script::parse_stream(&mut token_stream)?;
    let mut script_executor = ScriptExecutor::new(source.clone());
    let mut executor = Executor::new(e, &mut script_executor);
    script.statements.execute(&mut executor)?;
    Ok(())
}
