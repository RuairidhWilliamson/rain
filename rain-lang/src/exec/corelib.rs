use crate::{ast::fn_call::FnCall, error::RainError};

use super::{
    types::{function::Function, record::Record, RainValue},
    Executor,
};

pub fn core_lib() -> Record {
    let mut core_lib = Record::default();
    core_lib.insert(
        String::from("print"),
        RainValue::Function(Function::new_external(execute_print)),
    );
    core_lib
}

fn execute_print(
    _executor: &mut Executor,
    args: &[RainValue],
    _fn_call: &FnCall<'_>,
) -> Result<RainValue, RainError> {
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
