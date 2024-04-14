use afl::fuzz;
use rain_lang::exec::executor::ExecutorBuilder;

fn main() {
    fuzz!(|data: &str| {
        run(data);
    })
}

fn run(data: &str) {
    let mut executor = ExecutorBuilder::default().build();
    let source = rain_lang::source::Source::from(data);
    eprintln!("{data}");
    let _ = rain_lang::run(source, &mut executor);
}
