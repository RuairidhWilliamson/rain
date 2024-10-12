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
    ModuleFile,
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
            "module_file" => Some(Self::ModuleFile),
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
            Self::Print => print_implementation(arg_values),
            Self::Import => import_implementation(rir, cx, nid, fn_call, arg_values),
            Self::ModuleFile => module_file_implementation(cx, fn_call, arg_values),
        }
    }
}

fn print_implementation(arg_values: Vec<(NodeId, Value)>) -> ResultValue {
    let args: Vec<String> = arg_values
        .into_iter()
        .map(|(_, a)| format!("{a}"))
        .collect();
    println!("{}", args.join(" "));
    Ok(Value::new(()))
}

fn import_implementation(
    rir: &mut Rir,
    cx: &mut Cx,
    nid: NodeId,
    fn_call: &FnCall,
    arg_values: Vec<(NodeId, Value)>,
) -> ResultValue {
    match &arg_values[..] {
        [(relative_path_nid, relative_path_value)] => {
            let relative_path: &String = relative_path_value
                .downcast_ref()
                .ok_or_else(|| cx.nid_err(*relative_path_nid, RunnerError::GenericTypeError))?;
            let file = cx
                .module
                .file
                .as_ref()
                .unwrap()
                .parent()
                .unwrap()
                .join(relative_path)
                .map_err(|err| cx.nid_err(*relative_path_nid, err.into()))?;
            let resolved_path = file.resolve();
            let src = std::fs::read_to_string(&resolved_path)
                .map_err(|err| cx.nid_err(nid, RunnerError::ImportIOError(err)))?;
            let module = crate::ast::parser::parse_module(&src);
            let id = rir
                .insert_module(Some(file), src, module)
                .map_err(ErrorSpan::convert)?;
            Ok(Value::new(Module { id }))
        }
        [(area_nid, area_value), (absolute_path_nid, absolute_path_value)] => {
            let _absolute_path: &String = absolute_path_value
                .downcast_ref()
                .ok_or_else(|| cx.nid_err(*absolute_path_nid, RunnerError::GenericTypeError))?;
            todo!()
        }
        _ => Err(cx.err(fn_call.rparen_token, RunnerError::GenericTypeError)),
    }
}

fn module_file_implementation(
    cx: &mut Cx,
    fn_call: &FnCall,
    arg_values: Vec<(NodeId, Value)>,
) -> ResultValue {
    if !arg_values.is_empty() {
        return Err(cx.err(fn_call.rparen_token, RunnerError::GenericTypeError));
    }
    cx.module
        .file
        .clone()
        .map(Value::new)
        .ok_or_else(|| cx.err(fn_call.rparen_token, RunnerError::GenericTypeError))
}

fn local_area_implementation(
    cx: &mut Cx,
    fn_call: &FnCall,
    arg_values: Vec<(NodeId, Value)>,
) -> ResultValue {
    todo!()
}
