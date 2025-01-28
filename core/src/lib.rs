use rain_lang::afs::file_system::FileSystem as _;

pub mod config;
pub mod exe;
pub mod remote;

#[expect(clippy::result_unit_err)]
pub fn run_stderr(
    path: impl AsRef<std::path::Path>,
    declaration: &str,
    config: config::Config,
) -> Result<rain_lang::runner::value::Value, ()> {
    let file = rain_lang::afs::file::File::new_local(path.as_ref()).map_err(|err| {
        log::error!("could not get file: {err}");
    })?;
    let path = config.resolve_file(&file);
    let src = std::fs::read_to_string(&path).map_err(|err| {
        log::error!("could not read file {}: {err}", path.display());
    })?;
    let module = rain_lang::ast::parser::parse_module(&src);
    let mut ir = rain_lang::ir::Rir::new();
    let mid = ir.insert_module(file, src, module).map_err(|err| {
        log::error!("{}", err.resolve_ir(&ir));
    })?;
    let main = ir
        .resolve_global_declaration(mid, declaration)
        .ok_or_else(|| {
            log::error!("{declaration} declaration not found");
        })?;
    let mut runner = rain_lang::runner::Runner::new(ir, config);
    let value = runner.evaluate_and_call(main).map_err(|err| {
        log::error!("{}", err.resolve_ir(&runner.rir));
    })?;
    if value.rain_type_id() == rain_lang::runner::value::RainTypeId::Error {
        log::error!("{value:?}");
        return Err(());
    }
    Ok(value)
}

pub fn find_root_rain() -> Option<std::path::PathBuf> {
    let mut directory = std::env::current_dir().ok()?;
    loop {
        let p = directory.join("root.rain");
        if p.try_exists().is_ok_and(|b| b) {
            return Some(p);
        }
        if !directory.pop() {
            return None;
        }
    }
}
