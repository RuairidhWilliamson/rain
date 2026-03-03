#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|src: &str| {
    let _ = rain_lang::ast::ts_parser::parse_module(src);
});
