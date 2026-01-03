use rain_core::{CoreError, cache::Cache, config::Config, driver::DriverImpl};

fn run_error(path: &str) -> CoreError {
    let driver = DriverImpl::new(Config::default());
    let cache = Cache::default();
    rain_core::run(path, "main", &cache, &driver).unwrap_err()
}

/*
#[test]
fn conflicting_declarations() {
    let res = run("tests/errors/conflicting_declarations.rain");
    let _ = res;
    // TODO: Need to add a "type check" phase to make this work
    // match res {
    //     Ok(_) => panic!("should have errored but did not"),
    //     Err(CoreError::Other(s)) => panic!("wrong kind of error: {s}"),
    //     Err(CoreError::LangError(_lang_err)) => (),
    // }
}
*/

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
}
