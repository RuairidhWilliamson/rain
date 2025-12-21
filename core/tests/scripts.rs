use std::path::Path;

use rain_lang::runner::value::Value;

fn run(path: impl AsRef<Path>) -> Result<Value, ()> {
    rain_core::run_stderr(path, "main")
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
}
