use rain_core::{CoreError, cache::Cache, config::Config, driver::DriverImpl};

fn run_error(path: &str) -> CoreError {
    let driver = DriverImpl::new(Config::default());
    let cache = Cache::default();
    let mut err =
        rain_core::run(path, "main", &cache, &driver).expect_err("run should produce an error");
    if let CoreError::LangError(owned_resolved_error) = &mut err {
        // Back traces can contain generated filepaths which are unstable for snapshots
        owned_resolved_error
            .trace
            .iter_mut()
            .for_each(|(s, _, _)| *s = String::from("<hidden>"));
        owned_resolved_error.file_name = String::from("<hidden>");
    }
    err
}

macro_rules! tests {
    ($($name:ident,)*) => {
        $(
        #[test]
        fn $name() {
            insta::assert_snapshot!(run_error(concat!("tests/errors/", stringify!($name), ".rain")));
        }
        )*
    };
}

tests! {
    fail_type_check,
    fail_let_type_check,
    fail_let_destructure_type_check,
    record_type_check,
    private_declaration,
    throw,
    destructures,
    conflicting_overlay_files,
    conflicting_declarations,
    conflicting_destructure_declarations,
    underscore_start,
}
