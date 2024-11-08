#![allow(clippy::unnecessary_wraps, clippy::needless_pass_by_value)]

use std::path::{Path, PathBuf};

use crate::{
    area::{AbsolutePathBuf, File, FileArea, GeneratedFileArea, PathError},
    ast::{FnCall, NodeId},
    config::Config,
    ir::Rir,
    runner::value::RainList,
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
    Args,
    Run,
    EscapeBin,
    MainCommands,
    Unit,
    GetArea,
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
            "args" => Some(Self::Args),
            "run" => Some(Self::Run),
            "escape_bin" => Some(Self::EscapeBin),
            "main_commands" => Some(Self::MainCommands),
            "unit" => Some(Self::Unit),
            "get_area" => Some(Self::GetArea),
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
        let icx = InternalCx {
            config,
            rir,
            cx,
            nid,
            fn_call,
            arg_values,
        };
        match self {
            Self::Print => print_implementation(icx),
            Self::GetFile => get_file_implementation(icx),
            Self::Import => import_implementation(icx),
            Self::ModuleFile => module_file_implementation(icx),
            Self::LocalArea => local_area_implementation(icx),
            Self::Extract => extract_implementation(icx),
            Self::Args => args_implementation(icx),
            Self::Run => run_implementation(icx),
            Self::EscapeBin => escape_bin(icx),
            Self::MainCommands => main_commands_helper(icx),
            Self::Unit => unit(icx),
            Self::GetArea => get_area(icx),
        }
    }
}

struct InternalCx<'a, 'b> {
    config: &'a Config,
    rir: &'a mut Rir,
    cx: &'a mut Cx<'b>,
    nid: NodeId,
    fn_call: &'a FnCall,
    arg_values: Vec<(NodeId, Value)>,
}

fn print_implementation(icx: InternalCx) -> ResultValue {
    let args: Vec<String> = icx
        .arg_values
        .into_iter()
        .map(|(_, a)| format!("{a}"))
        .collect();
    println!("{}", args.join(" "));
    Ok(Value::new(()))
}

fn get_file_implementation(icx: InternalCx) -> ResultValue {
    match &icx.arg_values[..] {
        [(relative_path_nid, relative_path_value)] => {
            let relative_path: &String = relative_path_value.downcast_ref().ok_or_else(|| {
                icx.cx
                    .nid_err(*relative_path_nid, RunnerError::GenericTypeError)
            })?;
            let file = icx
                .cx
                .module
                .file
                .parent()
                .ok_or_else(|| icx.cx.nid_err(icx.nid, PathError::NoParentDirectory.into()))?
                .join(relative_path)
                .map_err(|err| icx.cx.nid_err(*relative_path_nid, err.into()))?;
            if !file.exists(icx.config).map_err(|err| {
                icx.cx
                    .nid_err(*relative_path_nid, RunnerError::AreaIOError(err))
            })? {
                return Err(icx
                    .cx
                    .nid_err(*relative_path_nid, RunnerError::GenericTypeError));
            }
            Ok(Value::new(file))
        }
        [(area_nid, area_value), (absolute_path_nid, absolute_path_value)] => {
            let area: &FileArea = area_value
                .downcast_ref()
                .ok_or_else(|| icx.cx.nid_err(*area_nid, RunnerError::GenericTypeError))?;
            let absolute_path: &String = absolute_path_value.downcast_ref().ok_or_else(|| {
                icx.cx
                    .nid_err(*absolute_path_nid, RunnerError::GenericTypeError)
            })?;
            let file = File::new(area.clone(), absolute_path)
                .map_err(|err| icx.cx.nid_err(icx.nid, err.into()))?;
            if !file.exists(icx.config).map_err(|err| {
                icx.cx
                    .nid_err(*absolute_path_nid, RunnerError::AreaIOError(err))
            })? {
                return Err(icx
                    .cx
                    .nid_err(*absolute_path_nid, RunnerError::GenericTypeError));
            }
            Ok(Value::new(file))
        }
        _ => Err(icx
            .cx
            .err(icx.fn_call.rparen_token, RunnerError::GenericTypeError)),
    }
}

fn import_implementation(icx: InternalCx) -> ResultValue {
    match &icx.arg_values[..] {
        [(file_nid, file_value)] => {
            let file: &File = file_value
                .downcast_ref()
                .ok_or_else(|| icx.cx.nid_err(*file_nid, RunnerError::GenericTypeError))?;
            let resolved_path = file.resolve(icx.config);
            let src = std::fs::read_to_string(&resolved_path)
                .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::ImportIOError(err)))?;
            let module = crate::ast::parser::parse_module(&src);
            let id = icx
                .rir
                .insert_module(file.clone(), src, module)
                .map_err(ErrorSpan::convert)?;
            Ok(Value::new(Module { id }))
        }
        _ => Err(icx
            .cx
            .err(icx.fn_call.rparen_token, RunnerError::GenericTypeError)),
    }
}

fn module_file_implementation(icx: InternalCx) -> ResultValue {
    if !icx.arg_values.is_empty() {
        return Err(icx
            .cx
            .err(icx.fn_call.rparen_token, RunnerError::GenericTypeError));
    }
    Ok(Value::new(icx.cx.module.file.clone()))
}

fn local_area_implementation(icx: InternalCx) -> ResultValue {
    let FileArea::Local(current_area_path) = &icx.cx.module.file.area else {
        return Err(icx.cx.nid_err(icx.nid, RunnerError::IllegalLocalArea));
    };
    let (path_nid, path_value) = icx
        .arg_values
        .first()
        .ok_or_else(|| icx.cx.nid_err(icx.nid, RunnerError::GenericTypeError))?;
    let path: &String = path_value
        .downcast_ref()
        .ok_or_else(|| icx.cx.nid_err(*path_nid, RunnerError::GenericTypeError))?;
    let area_path = current_area_path.join(path);
    let area_path = AbsolutePathBuf::try_from(area_path.as_path())
        .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::AreaIOError(err)))?;
    let metadata = std::fs::metadata(&*area_path)
        .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::AreaIOError(err)))?;
    if metadata.is_file() {
        return Err(icx.cx.nid_err(icx.nid, RunnerError::GenericTypeError));
    }
    Ok(Value::new(FileArea::Local(area_path)))
}

fn extract_implementation(icx: InternalCx) -> ResultValue {
    match &icx.arg_values[..] {
        [(file_nid, file_value)] => {
            let file: &File = file_value
                .downcast_ref()
                .ok_or_else(|| icx.cx.nid_err(*file_nid, RunnerError::GenericTypeError))?;
            let resolved_path = file.resolve(icx.config);
            let gen_area = GeneratedFileArea::new();
            let area = FileArea::Generated(gen_area);
            let output_dir = File::new(area.clone(), "/")
                .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::PathError(err)))?;
            let output_dir_path = output_dir.resolve(icx.config);
            std::fs::create_dir_all(&output_dir_path)
                .map_err(|err| icx.cx.nid_err(*file_nid, RunnerError::AreaIOError(err)))?;
            let f = std::fs::File::open(resolved_path)
                .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::AreaIOError(err)))?;
            let mut zip = zip::read::ZipArchive::new(f)
                .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::ZipError(err)))?;
            for i in 0..zip.len() {
                let mut zip_file = zip
                    .by_index(i)
                    .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::ZipError(err)))?;
                let Some(name) = zip_file.enclosed_name() else {
                    continue;
                };
                let mut out = std::fs::File::create_new(output_dir_path.join(name))
                    .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::AreaIOError(err)))?;
                std::io::copy(&mut zip_file, &mut out)
                    .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::AreaIOError(err)))?;
            }
            Ok(Value::new(area))
        }
        _ => Err(icx
            .cx
            .err(icx.fn_call.rparen_token, RunnerError::GenericTypeError)),
    }
}

fn args_implementation(_icx: InternalCx) -> ResultValue {
    let args: Vec<_> = std::env::args().skip(1).map(Value::new).collect();
    Ok(Value::new(RainList(args)))
}

fn run_implementation(icx: InternalCx) -> ResultValue {
    match &icx.arg_values[..] {
        [(area_nid, area_value), (file_nid, file_value), args @ ..] => {
            let area = FileArea::Generated(GeneratedFileArea::new());
            let output_dir = File::new(area.clone(), "/")
                .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::PathError(err)))?;
            let output_dir_path = output_dir.resolve(icx.config);
            match area_value.rain_type_id() {
                RainTypeId::Unit => {
                    std::fs::create_dir_all(&output_dir_path)
                        .map_err(|err| icx.cx.nid_err(*file_nid, RunnerError::AreaIOError(err)))?;
                }
                RainTypeId::FileArea => {
                    let area: &FileArea = area_value
                        .downcast_ref()
                        .ok_or_else(|| icx.cx.nid_err(*area_nid, RunnerError::GenericTypeError))?;
                    let input_dir = File::new(area.clone(), "/")
                        .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::PathError(err)))?;
                    let input_dir_path = input_dir.resolve(icx.config);
                    dircpy::copy_dir(input_dir_path, &output_dir_path)
                        .map_err(|err| icx.cx.nid_err(*area_nid, RunnerError::AreaIOError(err)))?;
                }
                _ => Err(icx.cx.nid_err(*area_nid, RunnerError::GenericTypeError))?,
            }
            let file: &File = file_value
                .downcast_ref()
                .ok_or_else(|| icx.cx.nid_err(*file_nid, RunnerError::GenericTypeError))?;
            let resolved_path = file.resolve(icx.config);
            let args = args
                .iter()
                .map(|(nid, value)| match value.rain_type_id() {
                    RainTypeId::String => Ok(value
                        .downcast_ref::<String>()
                        .ok_or_else(|| icx.cx.nid_err(*nid, RunnerError::GenericTypeError))?
                        .to_string()),
                    RainTypeId::File => Ok(value
                        .downcast_ref::<File>()
                        .ok_or_else(|| icx.cx.nid_err(*nid, RunnerError::GenericTypeError))?
                        .resolve(icx.config)
                        .display()
                        .to_string()),
                    _ => todo!(),
                })
                .collect::<Result<Vec<String>, ErrorSpan<RunnerError>>>()?;
            let mut cmd = std::process::Command::new(resolved_path);
            cmd.current_dir(output_dir_path);
            cmd.args(args);
            eprintln!("Running {cmd:?}");
            cmd.status()
                .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::AreaIOError(err)))?;
            Ok(Value::new(area))
        }
        _ => Err(icx
            .cx
            .err(icx.fn_call.rparen_token, RunnerError::GenericTypeError)),
    }
}

#[cfg(target_family = "unix")]
const PATH_SEPARATOR: char = ':';
#[cfg(target_family = "windows")]
const PATH_SEPARATOR: char = ';';

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

fn escape_bin(icx: InternalCx) -> ResultValue {
    match &icx.arg_values[..] {
        [(name_nid, name_value)] => {
            let name: &String = name_value
                .downcast_ref()
                .ok_or_else(|| icx.cx.nid_err(*name_nid, RunnerError::GenericTypeError))?;
            let path = std::env::var("PATH")
                .map_err(|_| icx.cx.nid_err(icx.nid, RunnerError::GenericTypeError))?
                .split(PATH_SEPARATOR)
                .find_map(|p| find_bin_in_dir(Path::new(p), name))
                .ok_or_else(|| icx.cx.nid_err(icx.nid, RunnerError::GenericTypeError))?;
            let f = File::new(FileArea::Escape, path.to_string_lossy().as_ref())
                .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::PathError(err)))?;
            Ok(Value::new(f))
        }
        _ => Err(icx
            .cx
            .err(icx.fn_call.rparen_token, RunnerError::GenericTypeError)),
    }
}

fn main_commands_helper(icx: InternalCx) -> ResultValue {
    let command = std::env::args()
        .nth(1)
        .ok_or_else(|| icx.cx.nid_err(icx.nid, RunnerError::GenericTypeError))?;
    let (_, command_value) = icx
        .arg_values
        .into_iter()
        .find(|(nid, _)| {
            let crate::ast::Node::Ident(tls) = icx.cx.module.get(*nid) else {
                todo!("not an ident")
            };
            tls.0.span.contents(&icx.cx.module.src) == command
        })
        .ok_or_else(|| icx.cx.nid_err(icx.nid, RunnerError::GenericTypeError))?;
    Ok(command_value)
}

fn unit(_icx: InternalCx) -> ResultValue {
    Ok(Value::new(()))
}

fn get_area(icx: InternalCx) -> ResultValue {
    match &icx.arg_values[..] {
        [(file_nid, file_value)] => {
            let file: &File = file_value
                .downcast_ref()
                .ok_or_else(|| icx.cx.nid_err(*file_nid, RunnerError::GenericTypeError))?;
            Ok(Value::new(file.area.clone()))
        }
        _ => Err(icx
            .cx
            .err(icx.fn_call.rparen_token, RunnerError::GenericTypeError)),
    }
}
