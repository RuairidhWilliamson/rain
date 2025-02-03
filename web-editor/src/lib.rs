#![allow(
    clippy::missing_panics_doc,
    clippy::unwrap_used,
    unsafe_code,
    unexpected_cfgs
)]

use std::sync::Mutex;

use rain_lang::{
    afs::{area::FileArea, file::File},
    driver::DriverTrait,
    runner::{cache::Cache, error::RunnerError},
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    // Use `js_namespace` here to bind `console.log(..)` instead of just
    // `log(..)`
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen]
pub fn init_panic_hook() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
}

#[wasm_bindgen]
pub fn run_source(source: String) -> Result<ExecuteOutput, String> {
    let file_system = FileSystemImpl::default();
    let module = rain_lang::ast::parser::parse_module(&source);
    let mut ir = rain_lang::ir::Rir::new();
    let file = File::new(FileArea::Escape, "/main.rain");
    let mid = ir
        .insert_module(file, source, module)
        .map_err(|err| format!("load module failed: {}", err.resolve_ir(&ir)))?;
    let main = ir
        .resolve_global_declaration(mid, "main")
        .ok_or_else(|| "no main item found".to_owned())?;
    let mut cache = Cache::new(rain_lang::runner::cache::CACHE_SIZE);
    let mut runner = rain_lang::runner::Runner::new(ir, &mut cache, &file_system);
    let value = runner
        .evaluate_and_call(main)
        .map_err(|err| format!("evaluate error: {}", err.resolve_ir(&runner.ir)))?;
    let prints = runner.driver.prints.lock().unwrap();
    Ok(ExecuteOutput {
        prints: prints.join("\n"),
        output: value.to_string(),
    })
}

#[wasm_bindgen(getter_with_clone)]
pub struct ExecuteOutput {
    pub prints: String,
    pub output: String,
}

#[derive(Default)]
struct FileSystemImpl {
    prints: Mutex<Vec<String>>,
}

impl DriverTrait for FileSystemImpl {
    fn resolve_file(&self, _file: &File) -> std::path::PathBuf {
        todo!()
    }

    fn exists(&self, _file: &File) -> Result<bool, std::io::Error> {
        todo!()
    }

    fn escape_bin(&self, _name: &str) -> Option<std::path::PathBuf> {
        todo!()
    }

    fn print(&self, message: String) {
        log(&message);
        self.prints.lock().unwrap().push(message);
    }

    fn extract(&self, _file: &File) -> Result<FileArea, Box<dyn std::error::Error>> {
        todo!()
    }

    fn run(
        &self,
        _area: Option<&FileArea>,
        _bin: &File,
        _args: Vec<String>,
    ) -> Result<rain_lang::driver::RunStatus, RunnerError> {
        todo!()
    }

    fn download(&self, _url: &str) -> Result<File, RunnerError> {
        todo!()
    }
}
