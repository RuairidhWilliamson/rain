use file_system::FileSystemImpl;
use rain_lang::{
    afs::file_system::FileSystemTrait as _,
    error::OwnedResolvedError,
    runner::cache::{Cache, CACHE_SIZE},
};

pub mod config;
pub mod exe;
pub mod file_system;
pub mod ipc;
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

// TODO: Get rid of unwraps
#[expect(clippy::unwrap_used)]
pub fn run(
    path: impl AsRef<std::path::Path>,
    declaration: &str,
    cache: &mut Cache,
    file_system: &FileSystemImpl,
) -> Result<rain_lang::runner::value::Value, OwnedResolvedError> {
    let file = rain_lang::afs::file::File::new_local(path.as_ref()).unwrap();
    let path = file_system.resolve_file(&file);
    let src = std::fs::read_to_string(&path).unwrap();
    let module = rain_lang::ast::parser::parse_module(&src);
    let mut ir = rain_lang::ir::Rir::new();
    let mid = ir
        .insert_module(file, src, module)
        .map_err(|err| err.resolve_ir(&ir).into_owned())?;
    let main = ir.resolve_global_declaration(mid, declaration).unwrap();
    let mut runner = rain_lang::runner::Runner::new(ir, cache, file_system);
    let value = runner.evaluate_and_call(main).unwrap();
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
