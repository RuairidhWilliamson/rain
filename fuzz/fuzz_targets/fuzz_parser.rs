#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|src: &str| {
    let _ = rain_lang::ast::parser::parse_module_inner(src);
});
