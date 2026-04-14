#![expect(clippy::print_stderr)]

use std::path::Path;

use rain_core::cache::Cache;
use rain_lang::runner::value::Value;
use test_log::test;

fn run(path: impl AsRef<Path>) -> Result<Value, ()> {
    let driver = rain_core::driver::DriverImpl::new(rain_core::config::Config::default());
    let cache = Cache {
        verification: true,
        ..Cache::default()
    };
    rain_core::run(path, "main", &cache, &driver).map_err(|err| {
        eprintln!("{err}");
    })
}

macro_rules! tests {
    ($($name:ident,)*) => {
        $(
        #[test]
        fn $name() {
            insta::assert_debug_snapshot!(run(concat!("tests/scripts/", stringify!($name), ".rain")).unwrap());
        }
        )*
    };
}

tests! {
    utf8,
    fib,
    local_var,
    fn_call,
    internal_print,
    internal_import,
    underscore,
    equality,
    create_area,
    cache_deps,
    strings,
    closure,
    complex_closures,
    string_add,
    addition,
    binary_operators,
    type_checks,
    internal,
    local_type_spec,
    type_check_inner,
    raw_string,
    unary_operators,
    import,
    destructure,
    destructure_import,
    any_type,
    record_type_check,
    generated_vs_local,
    trailing_commas,
    format_string,
    closure_disambiguation,
}
