use crate::{
    ast::{function_call::FnCall, Ast as _},
    error::RainError,
};

use super::{
    types::{function::FunctionArguments, RainValue},
    ExecError,
};

pub fn extract_arg(
    args: &FunctionArguments,
    name: &str,
    position: Option<usize>,
    fn_call: Option<&FnCall>,
) -> Result<RainValue, RainError> {
    if position.is_some() {
        todo!("implement positional arg extraction");
    }
    let (_, v) = args
        .iter()
        .filter_map(|(n, v)| n.as_ref().map(|n| (n, v)))
        .find(|(n, _)| n.name == name)
        .ok_or_else(|| {
            RainError::new(
                ExecError::MissingArg {
                    arg_name: String::from(name),
                },
                fn_call.unwrap().span(),
            )
        })?;
    Ok(v.clone())
}
