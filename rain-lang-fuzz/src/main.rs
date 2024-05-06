use afl::fuzz;
use rain_lang::{executor::builder::ExecutorBuilder, path::Workspace};

fn main() {
    fuzz!(|data: &str| {
        run(data);
    })
}

fn run(data: &str) {
    let workspace = Workspace::new_local_cwd().unwrap();
    let mut executor = ExecutorBuilder::default().build(workspace);
    let workspace = Workspace::Local(std::env::current_dir().unwrap());
    let source = rain_lang::source::Source::new_evaluated(workspace.new_path("."), data.to_owned());
    eprintln!("{data}");
    let _ = rain_lang::run(source, &mut executor);
}
