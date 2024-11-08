use runner::value::RainTypeId;

pub mod append_vec;
pub mod area;
pub mod ast;
pub mod config;
pub mod error;
pub mod ir;
pub mod local_span;
pub mod runner;
pub mod span;
pub mod tokens;

#[expect(clippy::result_unit_err)]
pub fn run_stderr(
    path: impl AsRef<std::path::Path>,
    declaration: &str,
    config: config::Config,
) -> Result<runner::value::Value, ()> {
    let file = area::File::new_local(path.as_ref()).map_err(|err| {
        eprintln!("could not get file: {err}");
    })?;
    let path = file.resolve(&config);
    let src = std::fs::read_to_string(&path).map_err(|err| {
        eprintln!("could not read file {}: {err}", path.display());
    })?;
    let module = ast::parser::parse_module(&src);
    let mut ir = ir::Rir::new();
    let mid = ir.insert_module(file, src, module).map_err(|err| {
        eprintln!("{}", err.resolve_ir(&ir));
    })?;
    let main = ir
        .resolve_global_declaration(mid, declaration)
        .ok_or_else(|| {
            eprintln!("{declaration} declaration not found");
        })?;
    let mut runner = runner::Runner::new(config, ir);
    let value = runner.evaluate_and_call(main).map_err(|err| {
        eprintln!("{}", err.resolve_ir(&runner.rir));
    })?;
    if value.rain_type_id() == RainTypeId::Error {
        eprintln!("{value:?}");
        return Err(());
    }
    Ok(value)
}
