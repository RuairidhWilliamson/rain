use crate::{
    ast::{FnCall, NodeId},
    ir::Rir,
    span::ErrorSpan,
};

use super::{
    error::RunnerError,
    value::{Module, RainTypeId, Value, ValueInner},
    Cx, ResultValue,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InternalFunction {
    Print,
    Import,
}

impl ValueInner for InternalFunction {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::InternalFunction
    }
}

impl InternalFunction {
    pub fn evaluate_internal_function_name(name: &str) -> Option<Self> {
        match name {
            "print" => Some(Self::Print),
            "import" => Some(Self::Import),
            _ => None,
        }
    }

    pub fn call_internal_function(
        self,
        rir: &mut Rir,
        cx: &mut Cx,
        nid: NodeId,
        fn_call: &FnCall,
        arg_values: Vec<(NodeId, Value)>,
    ) -> ResultValue {
        match self {
            Self::Print => {
                let args: Vec<String> = arg_values
                    .into_iter()
                    .map(|(_, a)| format!("{a}"))
                    .collect();
                println!("{}", args.join(" "));
                Ok(Value::new(()))
            }
            Self::Import => {
                let import_target = arg_values
                    .first()
                    .ok_or_else(|| cx.err(fn_call.rparen_token, RunnerError::GenericTypeError))?;
                let import_path: &String = import_target
                    .1
                    .downcast_ref()
                    .ok_or_else(|| cx.nid_err(import_target.0, RunnerError::GenericTypeError))?;
                let resolved_path = cx
                    .module
                    .path
                    .as_ref()
                    .ok_or_else(|| cx.nid_err(nid, RunnerError::ImportResolve))?
                    .parent()
                    .ok_or_else(|| cx.nid_err(nid, RunnerError::ImportResolve))?
                    .join(import_path);
                let src = std::fs::read_to_string(&resolved_path)
                    .map_err(|err| cx.nid_err(nid, RunnerError::ImportIOError(err)))?;
                let module = crate::ast::parser::parse_module(&src);
                let id = rir
                    .insert_module(Some(resolved_path), src, module)
                    .map_err(ErrorSpan::convert)?;
                Ok(Value::new(Module { id }))
            }
        }
    }
}
