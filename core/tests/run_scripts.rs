#![expect(clippy::print_stderr)]

use std::path::Path;

use rain_core::{CoreError, cache::Cache};
use rain_lang::error::OwnedTraceEntry;
use tracing_test::traced_test;

fn run(path: impl AsRef<Path>) -> String {
    let mut driver = rain_core::driver::DriverImpl::new(rain_core::config::Config::default());
    driver.print_handler = Some(Box::new(|s: &str| {
        eprintln!("print: {s}");
    }));
    let cache = Cache {
        verification: true,
        ..Cache::default()
    };
    let res = rain_core::run(path, "main", &cache, &driver).map_err(|mut err| {
        if let CoreError::LangError(owned_resolved_error) = &mut err {
            // Back traces can contain generated filepaths which are unstable for snapshots
            owned_resolved_error
                .trace
                .iter_mut()
                .for_each(|OwnedTraceEntry { filename, .. }| *filename = String::from("<hidden>"));
            owned_resolved_error.file_name = String::from("<hidden>");
        }
        err
    });
    match res {
        Ok(v) => {
            format!("Ok\n{v:#?}")
        }
        Err(err) => {
            format!("Err\n{err}")
        }
    }
}

macro_rules! tests {
    ($($name:ident,)*) => {
        $(
        #[traced_test]
        #[test]
        fn $name() {
            insta::assert_snapshot!(run(concat!("tests/scripts/", stringify!($name), ".rain")));
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
    // Currently doesn't work correctly
    // create_unique,
    regex_replace,
    fail_type_check,
    fail_let_type_check,
    fail_let_destructure_type_check,
    record_type_check_err,
    private_declaration,
    throw,
    destructures,
    conflicting_overlay_files,
    conflicting_declarations,
    conflicting_destructure_declarations,
    underscore_start,
    invalid_internal,
    bool_type_check,
    check_type,
    check_return_type,
    check_invalid_return_type_constraint,
    check_conflicting_return_type,
    check_arg_count,
    check_arg_type,
    check_import_sugar,
    check_stdlib_sugar,
    // check_arg_contravariance,
    fail_union_type_check,
    function_composition,
    check_underscore,
}
