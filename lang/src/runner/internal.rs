#![allow(clippy::unnecessary_wraps, clippy::needless_pass_by_value)]

use crate::{
    area::{AbsolutePathBuf, File, FileArea, GeneratedFileArea, PathError},
    ast::{FnCall, NodeId},
    config::Config,
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
    GetFile,
    Import,
    ModuleFile,
    LocalArea,
    Extract,
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
            "get_file" => Some(Self::GetFile),
            "import" => Some(Self::Import),
            "module_file" => Some(Self::ModuleFile),
            "local_area" => Some(Self::LocalArea),
            "extract" => Some(Self::Extract),
            _ => None,
        }
    }

    pub fn call_internal_function(
        self,
        config: &Config,
        rir: &mut Rir,
        cx: &mut Cx,
        nid: NodeId,
        fn_call: &FnCall,
        arg_values: Vec<(NodeId, Value)>,
    ) -> ResultValue {
        match self {
            Self::Print => print_implementation(arg_values),
            Self::GetFile => get_file_implementation(cx, nid, fn_call, arg_values),
            Self::Import => import_implementation(config, rir, cx, nid, fn_call, arg_values),
            Self::ModuleFile => module_file_implementation(cx, fn_call, arg_values),
            Self::LocalArea => local_area_implementation(cx, nid, arg_values),
            Self::Extract => extract_implementation(config, cx, nid, fn_call, arg_values),
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

fn get_file_implementation(
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
                .parent()
                .ok_or_else(|| cx.nid_err(nid, PathError::NoParentDirectory.into()))?
                .join(relative_path)
                .map_err(|err| cx.nid_err(*relative_path_nid, err.into()))?;
            Ok(Value::new(file))
        }
        [(area_nid, area_value), (absolute_path_nid, absolute_path_value)] => {
            let area: &FileArea = area_value
                .downcast_ref()
                .ok_or_else(|| cx.nid_err(*area_nid, RunnerError::GenericTypeError))?;
            let absolute_path: &String = absolute_path_value
                .downcast_ref()
                .ok_or_else(|| cx.nid_err(*absolute_path_nid, RunnerError::GenericTypeError))?;
            let file = File::new(area.clone(), absolute_path)
                .map_err(|err| cx.nid_err(nid, err.into()))?;
            Ok(Value::new(file))
        }
        _ => Err(cx.err(fn_call.rparen_token, RunnerError::GenericTypeError)),
    }
}

fn import_implementation(
    config: &Config,
    rir: &mut Rir,
    cx: &mut Cx,
    nid: NodeId,
    fn_call: &FnCall,
    arg_values: Vec<(NodeId, Value)>,
) -> ResultValue {
    match &arg_values[..] {
        [(file_nid, file_value)] => {
            let file: &File = file_value
                .downcast_ref()
                .ok_or_else(|| cx.nid_err(*file_nid, RunnerError::GenericTypeError))?;
            let resolved_path = file.resolve(config);
            let src = std::fs::read_to_string(&resolved_path)
                .map_err(|err| cx.nid_err(nid, RunnerError::ImportIOError(err)))?;
            let module = crate::ast::parser::parse_module(&src);
            let id = rir
                .insert_module(file.clone(), src, module)
                .map_err(ErrorSpan::convert)?;
            Ok(Value::new(Module { id }))
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
    Ok(Value::new(cx.module.file.clone()))
}

fn local_area_implementation(
    cx: &mut Cx,
    nid: NodeId,
    arg_values: Vec<(NodeId, Value)>,
) -> ResultValue {
    let FileArea::Local(current_area_path) = &cx.module.file.area else {
        return Err(cx.nid_err(nid, RunnerError::IllegalLocalArea));
    };
    let (path_nid, path_value) = arg_values
        .first()
        .ok_or_else(|| cx.nid_err(nid, RunnerError::GenericTypeError))?;
    let path: &String = path_value
        .downcast_ref()
        .ok_or_else(|| cx.nid_err(*path_nid, RunnerError::GenericTypeError))?;
    let area_path = current_area_path.join(path);
    let area_path = AbsolutePathBuf::try_from(area_path.as_path())
        .map_err(|err| cx.nid_err(nid, RunnerError::AreaIOError(err)))?;
    let metadata = std::fs::metadata(&*area_path)
        .map_err(|err| cx.nid_err(nid, RunnerError::AreaIOError(err)))?;
    if metadata.is_file() {
        return Err(cx.nid_err(nid, RunnerError::GenericTypeError));
    }
    Ok(Value::new(FileArea::Local(area_path)))
}

fn extract_implementation(
    config: &Config,
    cx: &mut Cx,
    nid: NodeId,
    fn_call: &FnCall,
    arg_values: Vec<(NodeId, Value)>,
) -> ResultValue {
    match &arg_values[..] {
        [(file_nid, file_value)] => {
            let file: &File = file_value
                .downcast_ref()
                .ok_or_else(|| cx.nid_err(*file_nid, RunnerError::GenericTypeError))?;
            let resolved_path = file.resolve(config);
            let gen_area = GeneratedFileArea::new();
            let area = FileArea::Generated(gen_area);
            let output_dir = File::new(area.clone(), "/")
                .map_err(|err| cx.nid_err(nid, RunnerError::PathError(err)))?;
            let output_dir_path = output_dir.resolve(config);
            std::fs::create_dir_all(&output_dir_path)
                .map_err(|err| cx.nid_err(*file_nid, RunnerError::AreaIOError(err)))?;
            let f = std::fs::File::open(resolved_path)
                .map_err(|err| cx.nid_err(nid, RunnerError::AreaIOError(err)))?;
            let mut zip = zip::read::ZipArchive::new(f)
                .map_err(|err| cx.nid_err(nid, RunnerError::ZipError(err)))?;
            for i in 0..zip.len() {
                let mut zip_file = zip
                    .by_index(i)
                    .map_err(|err| cx.nid_err(nid, RunnerError::ZipError(err)))?;
                let Some(name) = zip_file.enclosed_name() else {
                    continue;
                };
                let mut out = std::fs::File::create_new(output_dir_path.join(name))
                    .map_err(|err| cx.nid_err(nid, RunnerError::AreaIOError(err)))?;
                std::io::copy(&mut zip_file, &mut out)
                    .map_err(|err| cx.nid_err(nid, RunnerError::AreaIOError(err)))?;
            }
            Ok(Value::new(area))
        }
        _ => Err(cx.err(fn_call.rparen_token, RunnerError::GenericTypeError)),
    }
}
