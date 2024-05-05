use std::ffi::OsStr;

use rain_lang::{exec::executor::ExecutorBuilder, path::Workspace, source::Source};

#[test]
fn run_all_test_scripts() {
    let test_scripts_dir = std::fs::read_dir("tests/scripts").unwrap();
    let workspace = Workspace::Local("tests/scripts".into());
    let mut error_count = 0;
    test_scripts_dir.for_each(|test_script| {
        let test_script = test_script.unwrap();
        let path = test_script.path();
        if path.extension() != Some(OsStr::new("rain")) {
            eprintln!("--- skipping {}", path.display());
            return;
        } else {
            eprintln!("--- running {}", path.display());
        }
        let mut executor = ExecutorBuilder::default().build(workspace.clone());
        let source =
            Source::new(&workspace.new_path(path.strip_prefix("tests/scripts").unwrap())).unwrap();
        if let Err(err) = rain_lang::run(source, &mut executor) {
            eprintln!("=== error: {err:#}");
            error_count += 1;
        }
    });
    if error_count > 0 {
        panic!("{error_count} errors");
    }
}
