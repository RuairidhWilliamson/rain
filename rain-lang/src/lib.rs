use exec::{
    executor::{Executor, GlobalExecutorBuilder},
    ExecCF,
};

pub mod ast;
pub mod error;
pub mod exec;
pub mod source;
pub mod span;
pub mod tokens;
mod utils;

pub fn run(source: &source::Source, e: GlobalExecutorBuilder) -> Result<(), error::ResolvedError> {
    match run_inner(source, e) {
        Ok(()) => Ok(()),
        Err(ExecCF::Return(_)) => todo!(),
        Err(ExecCF::RuntimeError(_)) => todo!(),
        Err(ExecCF::RainError(err)) => Err(err.resolve(source)),
    }
}

fn run_inner(source: &source::Source, builder: GlobalExecutorBuilder) -> Result<(), ExecCF> {
    // TODO: We should properly track the lifetime of the source code
    let s = source.source.to_owned().leak();
    let mut token_stream = tokens::peek_stream::PeekTokenStream::new(s);
    let script: ast::script::Script<'static> =
        ast::script::Script::parse_stream(&mut token_stream)?;
    let mut global_executor = builder.build();
    let mut executor = Executor::new(&mut global_executor);
    exec::execution::Execution::execute(&script, &mut executor)?;
    Ok(())
}
