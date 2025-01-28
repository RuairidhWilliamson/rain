use file_system::FileSystemImpl;
use rain_lang::{
    afs::file_system::FileSystemTrait as _,
    runner::cache::{Cache, CACHE_SIZE},
};

pub mod config;
pub mod exe;
pub mod file_system;
pub mod remote;

#[expect(clippy::result_unit_err)]
pub fn run_stderr(
    path: impl AsRef<std::path::Path>,
    declaration: &str,
    file_system: &FileSystemImpl,
) -> Result<rain_lang::runner::value::Value, ()> {
    let mut cache = Cache::new(CACHE_SIZE);
    run(path, declaration, &mut cache, file_system).map_err(|err| {
        log::error!("{err}");
    })
}

pub fn run(
    path: impl AsRef<std::path::Path>,
    declaration: &str,
    cache: &mut Cache,
    file_system: &FileSystemImpl,
) -> Result<rain_lang::runner::value::Value, String> {
    let file = rain_lang::afs::file::File::new_local(path.as_ref())
        .map_err(|err| format!("could not get file: {err}"))?;
    let path = file_system.resolve_file(&file);
    let src = std::fs::read_to_string(&path)
        .map_err(|err| format!("could not read file {}: {err}", path.display()))?;
    let module = rain_lang::ast::parser::parse_module(&src);
    let mut ir = rain_lang::ir::Rir::new();
    let mid = ir
        .insert_module(file, src, module)
        .map_err(|err| err.resolve_ir(&ir).to_string())?;
    let main = ir
        .resolve_global_declaration(mid, declaration)
        .ok_or_else(|| format!("{declaration} declaration not found"))?;
    let mut runner = rain_lang::runner::Runner::new(ir, cache, file_system);
    let value = runner
        .evaluate_and_call(main)
        .map_err(|err| err.resolve_ir(&runner.rir).to_string())?;
    if value.rain_type_id() == rain_lang::runner::value::RainTypeId::Error {
        return Err(format!("{value:?}"));
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
