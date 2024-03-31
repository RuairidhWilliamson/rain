use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

use rain_lang::{
    ast::{function_call::FnCall, Ast},
    error::RainError,
    exec::{
        executor::Executor,
        types::{function::Function, record::Record, RainType, RainValue},
        ExecCF, ExecError,
    },
};

pub fn std_escape_lib() -> Record {
    let mut std_escape_lib = Record::default();
    std_escape_lib.insert(
        String::from("bin"),
        RainValue::Function(Function::new_external(execute_bin)),
    );
    std_escape_lib
}

fn execute_bin(
    _executor: &mut Executor,
    args: &[RainValue],
    fn_call: Option<&FnCall<'_>>,
) -> Result<RainValue, ExecCF> {
    let [arg] = args else {
        return Err(RainError::new(
            ExecError::IncorrectArgCount {
                expected: (1..=1).into(),
                actual: args.len(),
            },
            fn_call.unwrap().span(),
        )
        .into());
    };
    let RainValue::String(name) = arg else {
        return Err(RainError::new(
            ExecError::UnexpectedType {
                expected: &[RainType::String],
                actual: arg.as_type(),
            },
            fn_call.unwrap().span(),
        )
        .into());
    };
    let path = find_bin_in_path(name).unwrap();
    Ok(RainValue::File(Rc::new(
        rain_lang::exec::types::file::File {
            kind: rain_lang::exec::types::file::FileKind::Escaped,
            path,
        },
    )))
}

fn find_bin_in_path(name: &str) -> Option<PathBuf> {
    std::env::var("PATH")
        .unwrap()
        .split(':')
        .find_map(|p| find_bin_in_dir(Path::new(p), name))
}

fn find_bin_in_dir(dir: &Path, name: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.find_map(|e| {
        let p = e.ok()?;
        if p.file_name().to_str()? == name {
            Some(p.path())
        } else {
            None
        }
    })
}
