use std::ffi::OsStr;

#[test]
fn run_all_test_scripts() {
    let test_scripts_dir = std::fs::read_dir("tests/scripts").unwrap();
    let mut error_count = 0;
    test_scripts_dir.for_each(|test_script| {
        let test_script = test_script.unwrap();
        let path = test_script.path();
        if path.extension() != Some(OsStr::new("rain")) {
            eprintln!("skipping {}", path.display());
            return;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        if let Err(err) = rain_lang::run(&path, &source) {
            eprintln!("{err:#}");
            error_count += 1;
        }
    });
    if error_count > 0 {
        panic!("{error_count} errors");
    }
}
