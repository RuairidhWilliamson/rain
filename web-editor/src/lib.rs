#![allow(clippy::missing_panics_doc, clippy::unwrap_used)]

use std::path::Path;

use rain_lang::afs::{file::File, file_system::FileSystem};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    // Use `js_namespace` here to bind `console.log(..)` instead of just
    // `log(..)`
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen]
pub fn run_source(source: String) -> String {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    let file_system = FileSystemImpl {};
    let module = rain_lang::ast::parser::parse_module(&source);
    let mut ir = rain_lang::ir::Rir::new();
    let file = File::new(rain_lang::afs::area::FileArea::Escape, "/main.rain").unwrap();
    let mid = ir.insert_module(file, source, module).unwrap();
    let main = ir.resolve_global_declaration(mid, "main").unwrap();
    let file_system = Box::new(file_system);
    let mut runner = rain_lang::runner::Runner::new(ir, file_system);
    let value = runner.evaluate_and_call(main).unwrap();
    value.to_string()
}

struct FileSystemImpl {}

impl FileSystem for FileSystemImpl {
    fn resolve_file(&self, _file: &File) -> std::path::PathBuf {
        todo!()
    }

    fn exists(&self, _file: &File) -> Result<bool, std::io::Error> {
        todo!()
    }

    fn escape_bin(&self, _name: &str) -> Option<std::path::PathBuf> {
        todo!()
    }
}
