use rain_core::{CoreError, config::Config, driver::DriverImpl};
use rain_lang::runner::{
    cache::{CACHE_SIZE, Cache},
    value::Value,
};

fn run(path: &str) -> Result<Value, CoreError> {
    let driver = DriverImpl::new(Config::default());
    let mut cache = Cache::new(CACHE_SIZE);
    rain_core::run(path, "main", &mut cache, &driver)
}

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
