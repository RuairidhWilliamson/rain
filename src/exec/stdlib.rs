use crate::error::RainError;

use super::{
    types::{function::Function, record::Record, RainValue},
    Executor,
};

pub fn std_lib() -> Record {
    let mut std_lib = Record::default();
    std_lib.insert(
        String::from("print"),
        RainValue::Function(Function::new_external(execute_print)),
    );
    std_lib.insert(String::from("escape"), RainValue::Record(std_escape_lib()));
    std_lib
}

fn execute_print(_executor: &mut Executor, args: &[RainValue]) -> Result<RainValue, RainError> {
    struct Args<'a>(&'a [RainValue]);
    impl std::fmt::Display for Args<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let Some((first, rest)) = self.0.split_first() else {
                return Ok(());
            };
            first.fmt(f)?;
            for a in rest {
                a.fmt(f)?;
                f.write_str(" ")?;
            }
            Ok(())
        }
    }
    let args = Args(args);
    println!("{args}");
    Ok(RainValue::Unit)
}

pub fn std_escape_lib() -> Record {
    let mut std_escape_lib = Record::default();
    std_escape_lib.insert(
        String::from("bin"),
        RainValue::Function(Function::new_external(execute_bin)),
    );
    std_escape_lib
}

fn execute_bin(_executor: &mut Executor, _args: &[RainValue]) -> Result<RainValue, RainError> {
    todo!("implement std.escape.bin")
}
