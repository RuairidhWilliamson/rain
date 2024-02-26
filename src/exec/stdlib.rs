use std::rc::Rc;

use crate::{error::RainError, exec::types::Unit};

use super::{
    types::{record::Record, DynValue, FnWrapper},
    Executor,
};

pub fn std_lib() -> Record {
    let mut std_lib = Record::default();
    std_lib.insert(
        String::from("print"),
        Rc::new(FnWrapper(Box::new(execute_print))),
    );
    std_lib
}

fn execute_print(_executor: &mut Executor, args: &[DynValue]) -> Result<DynValue, RainError> {
    struct Args<'a>(&'a [DynValue]);
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
    Ok(Rc::new(Unit))
}
