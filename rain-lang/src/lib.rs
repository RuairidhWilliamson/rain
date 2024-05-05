use ast::declaration::Declaration;
use error::RainError;
use exec::{
    executor::{BaseExecutor, Executor},
    script::ScriptExecutor,
    types::function::Function,
    ExecCF, ExecError,
};
use span::Span;

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
    let script_executor = ScriptExecutor::new(script, source.clone())?;
    let run_target = String::from("main");
    let f = script_executor
        .get(&run_target)
        .ok_or_else(|| RainError::new(ExecError::UnknownItem(run_target), Span::default()))?;
    let mut executor = Executor::new(e, &script_executor);
    let Declaration::FnDeclare(func) = f else {
        panic!("main is not a function");
    };
    Function::new(source.clone(), func.clone()).call(&mut executor, &[], None)?;
    Ok(())
}
