#![allow(clippy::unnecessary_wraps, clippy::needless_pass_by_value)]

use crate::{
    afs::{
        absolute::AbsolutePathBuf,
        area::{FileArea, GeneratedFileArea},
        error::PathError,
        file::File,
        file_system::FileSystem,
    },
    ast::{FnCall, NodeId},
    ir::Rir,
    runner::value_impl::{RainError, RainUnit},
    span::ErrorSpan,
};

use super::{
    error::RunnerError,
    value::{RainTypeId, Value, ValueInner},
    value_impl::{Module, RainList},
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
    Unit,
    GetArea,
    Download,
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
            "unit" => Some(Self::Unit),
            "get_area" => Some(Self::GetArea),
            "download" => Some(Self::Download),
            _ => None,
        }
    }

    pub fn call_internal_function(
        self,
        file_system: &dyn FileSystem,
        rir: &mut Rir,
        cx: &mut Cx,
        nid: NodeId,
        fn_call: &FnCall,
        arg_values: Vec<(NodeId, Value)>,
    ) -> ResultValue {
        let icx = InternalCx {
            file_system,
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
            Self::Unit => unit(icx),
            Self::GetArea => get_area(icx),
            Self::Download => download(icx),
        }
    }
}

struct InternalCx<'a, 'b> {
    file_system: &'a dyn FileSystem,
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
    icx.file_system.print(args.join(" "));
    Ok(Value::new(RainUnit))
}

fn get_file_implementation(icx: InternalCx) -> ResultValue {
    match &icx.arg_values[..] {
        [(relative_path_nid, relative_path_value)] => {
            let relative_path: &String = relative_path_value
                .downcast_ref_error(&[RainTypeId::String])
                .map_err(|err| icx.cx.nid_err(*relative_path_nid, err))?;
            let file = icx
                .cx
                .module
                .file
                .parent()
                .ok_or_else(|| icx.cx.nid_err(icx.nid, PathError::NoParentDirectory.into()))?
                .join(relative_path)
                .map_err(|err| icx.cx.nid_err(*relative_path_nid, err.into()))?;
            if !icx.file_system.exists(&file).map_err(|err| {
                icx.cx
                    .nid_err(*relative_path_nid, RunnerError::AreaIOError(err))
            })? {
                return Err(icx
                    .cx
                    .nid_err(*relative_path_nid, RunnerError::FileDoesNotExist));
            }
            Ok(Value::new(file))
        }
        [(area_nid, area_value), (absolute_path_nid, absolute_path_value)] => {
            let area: &FileArea = area_value
                .downcast_ref_error(&[RainTypeId::FileArea])
                .map_err(|err| icx.cx.nid_err(*area_nid, err))?;
            let absolute_path: &String = absolute_path_value
                .downcast_ref_error(&[RainTypeId::String])
                .map_err(|err| icx.cx.nid_err(*absolute_path_nid, err))?;
            let file = File::new(area.clone(), absolute_path)
                .map_err(|err| icx.cx.nid_err(icx.nid, err.into()))?;
            if !icx.file_system.exists(&file).map_err(|err| {
                icx.cx
                    .nid_err(*absolute_path_nid, RunnerError::AreaIOError(err))
            })? {
                return Err(icx
                    .cx
                    .nid_err(*absolute_path_nid, RunnerError::FileDoesNotExist));
            }
            Ok(Value::new(file))
        }
        _ => Err(icx.cx.err(
            icx.fn_call.rparen_token,
            RunnerError::IncorrectArgs {
                required: 1..=2,
                actual: icx.arg_values.len(),
            },
        )),
    }
}

fn import_implementation(icx: InternalCx) -> ResultValue {
    match &icx.arg_values[..] {
        [(file_nid, file_value)] => {
            let file: &File = file_value
                .downcast_ref_error(&[RainTypeId::File])
                .map_err(|err| icx.cx.nid_err(*file_nid, err))?;
            let resolved_path = icx.file_system.resolve_file(file);
            let src = std::fs::read_to_string(&resolved_path)
                .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::ImportIOError(err)))?;
            let module = crate::ast::parser::parse_module(&src);
            let id = icx
                .rir
                .insert_module(file.clone(), src, module)
                .map_err(ErrorSpan::convert)?;
            Ok(Value::new(Module { id }))
        }
        _ => Err(icx.cx.err(
            icx.fn_call.rparen_token,
            RunnerError::IncorrectArgs {
                required: 1..=1,
                actual: icx.arg_values.len(),
            },
        )),
    }
}

fn module_file_implementation(icx: InternalCx) -> ResultValue {
    if !icx.arg_values.is_empty() {
        return Err(icx.cx.err(
            icx.fn_call.rparen_token,
            RunnerError::IncorrectArgs {
                required: 0..=0,
                actual: icx.arg_values.len(),
            },
        ));
    }
    Ok(Value::new(icx.cx.module.file.clone()))
}

fn local_area_implementation(icx: InternalCx) -> ResultValue {
    let FileArea::Local(current_area_path) = &icx.cx.module.file.area else {
        return Err(icx.cx.nid_err(icx.nid, RunnerError::IllegalLocalArea));
    };
    let (path_nid, path_value) = icx.arg_values.first().ok_or_else(|| {
        icx.cx.nid_err(
            icx.nid,
            RunnerError::IncorrectArgs {
                required: 1..=1,
                actual: icx.arg_values.len(),
            },
        )
    })?;
    let path: &String = path_value
        .downcast_ref_error(&[RainTypeId::String])
        .map_err(|err| icx.cx.nid_err(*path_nid, err))?;
    let area_path = current_area_path.join(path);
    let area_path = AbsolutePathBuf::try_from(area_path.as_path())
        .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::AreaIOError(err)))?;
    let metadata = std::fs::metadata(&*area_path)
        .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::AreaIOError(err)))?;
    if metadata.is_file() {
        return Err(icx.cx.nid_err(icx.nid, RunnerError::GenericRunError));
    }
    Ok(Value::new(FileArea::Local(area_path)))
}

fn extract_implementation(icx: InternalCx) -> ResultValue {
    match &icx.arg_values[..] {
        [(file_nid, file_value)] => {
            let file: &File = file_value
                .downcast_ref_error(&[RainTypeId::File])
                .map_err(|err| icx.cx.nid_err(*file_nid, err))?;
            let area = icx
                .file_system
                .extract(file)
                .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::ExtractError(err)))?;
            Ok(Value::new(area))
        }
        _ => Err(icx.cx.err(
            icx.fn_call.rparen_token,
            RunnerError::IncorrectArgs {
                required: 1..=1,
                actual: icx.arg_values.len(),
            },
        )),
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
            let output_dir_path = icx.file_system.resolve_file(&output_dir);
            match area_value.rain_type_id() {
                RainTypeId::Unit => {
                    std::fs::create_dir_all(&output_dir_path)
                        .map_err(|err| icx.cx.nid_err(*file_nid, RunnerError::AreaIOError(err)))?;
                }
                RainTypeId::FileArea => {
                    todo!();
                    // let area: &FileArea = area_value
                    //     .downcast_ref_error(&[RainTypeId::FileArea])
                    //     .map_err(|err| icx.cx.nid_err(*area_nid, err))?;
                    // let input_dir = File::new(area.clone(), "/")
                    //     .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::PathError(err)))?;
                    // let input_dir_path = icx.file_system.resolve_file(&input_dir);
                    // dircpy::copy_dir(input_dir_path, &output_dir_path)
                    //     .map_err(|err| icx.cx.nid_err(*area_nid, RunnerError::AreaIOError(err)))?;
                }
                _ => Err(icx.cx.nid_err(*area_nid, RunnerError::GenericRunError))?,
            }
            let file: &File = file_value
                .downcast_ref_error(&[RainTypeId::File])
                .map_err(|err| icx.cx.nid_err(*file_nid, err))?;
            let resolved_path = icx.file_system.resolve_file(file);
            let args = args
                .iter()
                .map(|(nid, value)| match value.rain_type_id() {
                    RainTypeId::String => Ok(value
                        .downcast_ref_error::<String>(&[RainTypeId::String])
                        .map_err(|err| icx.cx.nid_err(*nid, err))?
                        .to_string()),
                    RainTypeId::File => Ok(icx
                        .file_system
                        .resolve_file(
                            value
                                .downcast_ref_error::<File>(&[RainTypeId::File])
                                .map_err(|err| icx.cx.nid_err(*nid, err))?,
                        )
                        .display()
                        .to_string()),
                    type_id => Err(icx.cx.nid_err(
                        *nid,
                        RunnerError::ExpectedType {
                            actual: type_id,
                            expected: &[RainTypeId::String, RainTypeId::File],
                        },
                    )),
                })
                .collect::<Result<Vec<String>, ErrorSpan<RunnerError>>>()?;
            let mut cmd = std::process::Command::new(resolved_path);
            cmd.current_dir(output_dir_path);
            cmd.args(args);
            // TODO: It would be nice to remove env vars but for the moment this causes too many problems
            // cmd.env_clear();
            log::debug!("Running {cmd:?}");
            let exit = cmd
                .status()
                .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::AreaIOError(err)))?;
            if !exit.success() {
                return Ok(Value::new(RainError("command failed".into())));
            }
            Ok(Value::new(area))
        }
        _ => Err(icx.cx.err(
            icx.fn_call.rparen_token,
            RunnerError::IncorrectArgs {
                required: 1..=2,
                actual: icx.arg_values.len(),
            },
        )),
    }
}

fn escape_bin(icx: InternalCx) -> ResultValue {
    match &icx.arg_values[..] {
        [(name_nid, name_value)] => {
            let name: &String = name_value
                .downcast_ref_error(&[RainTypeId::String])
                .map_err(|err| icx.cx.nid_err(*name_nid, err))?;
            let path = icx
                .file_system
                .escape_bin(name)
                .ok_or_else(|| icx.cx.nid_err(icx.nid, RunnerError::GenericRunError))?;
            let f = File::new(FileArea::Escape, path.to_string_lossy().as_ref())
                .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::PathError(err)))?;
            Ok(Value::new(f))
        }
        _ => Err(icx.cx.err(
            icx.fn_call.rparen_token,
            RunnerError::IncorrectArgs {
                required: 1..=1,
                actual: icx.arg_values.len(),
            },
        )),
    }
}

fn unit(_icx: InternalCx) -> ResultValue {
    Ok(Value::new(RainUnit))
}

fn get_area(icx: InternalCx) -> ResultValue {
    match &icx.arg_values[..] {
        [(file_nid, file_value)] => {
            let file: &File = file_value
                .downcast_ref_error(&[RainTypeId::File])
                .map_err(|err| icx.cx.nid_err(*file_nid, err))?;
            Ok(Value::new(file.area.clone()))
        }
        _ => Err(icx.cx.err(
            icx.fn_call.rparen_token,
            RunnerError::IncorrectArgs {
                required: 1..=1,
                actual: icx.arg_values.len(),
            },
        )),
    }
}

// TODO: Remove unwraps
// #[expect(clippy::unwrap_used)]
fn download(_icx: InternalCx) -> ResultValue {
    todo!()
    // let client = reqwest::blocking::Client::new();
    // match &icx.arg_values[..] {
    //     [(url_nid, url_value)] => {
    //         let url: &String = url_value
    //             .downcast_ref_error(&[RainTypeId::String])
    //             .map_err(|err| icx.cx.nid_err(*url_nid, err))?;
    //         let request = client
    //             .request(reqwest::Method::GET, url)
    //             // .header(
    //             //     reqwest::header::IF_NONE_MATCH,
    //             //     "\"3b22f9fe438383527860677d34196a03d388c34822b85064d0e0f2a1683c91dc\"",
    //             // )
    //             .build()
    //             .unwrap();
    //         log::debug!("Sending request {request:?}");
    //         let mut response = client.execute(request).unwrap();
    //         log::debug!("Received response {response:?}");
    //         let gen_area = GeneratedFileArea::new();
    //         let area = FileArea::Generated(gen_area);
    //         let output = File::new(area, "/download")
    //             .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::PathError(err)))?;
    //         let output_path = output.resolve(icx.config);
    //         let output_dir_path = output_path.parent().unwrap();
    //         std::fs::create_dir_all(output_dir_path)
    //             .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::AreaIOError(err)))?;
    //         let mut out = std::fs::File::create_new(output_path)
    //             .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::AreaIOError(err)))?;
    //         std::io::copy(&mut response, &mut out)
    //             .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::AreaIOError(err)))?;
    //         Ok(Value::new(output))
    //     }
    //     _ => Err(icx.cx.err(
    //         icx.fn_call.rparen_token,
    //         RunnerError::IncorrectArgs {
    //             required: 1..=1,
    //             actual: icx.arg_values.len(),
    //         },
    //     )),
    // }
}
