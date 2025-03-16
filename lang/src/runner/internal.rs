#![allow(clippy::unnecessary_wraps, clippy::needless_pass_by_value)]

use std::{collections::HashMap, ops::RangeInclusive};

use num_bigint::BigInt;

use crate::{
    afs::{absolute::AbsolutePathBuf, area::FileArea, error::PathError, file::File},
    ast::{FnCall, NodeId},
    driver::{DownloadStatus, DriverTrait},
    ir::Rir,
    runner::value_impl::RainUnit,
    span::ErrorSpan,
};

use super::{
    Cx, Result, ResultValue,
    cache::CacheStrategy,
    error::RunnerError,
    value::{RainTypeId, Value, ValueInner},
    value_impl::{Module, RainInteger, RainList, RainRecord},
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
    Throw,
    ReadFile,
    Sha256,
    BytesToString,
    ParseToml,
}

impl std::fmt::Display for InternalFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

impl ValueInner for InternalFunction {
    fn rain_type_id(&self) -> RainTypeId {
        RainTypeId::InternalFunction
    }
}

impl InternalFunction {
    pub fn evaluate_internal_function_name(name: &str) -> Option<Self> {
        match name {
            "_print" => Some(Self::Print),
            "_get_file" => Some(Self::GetFile),
            "_import" => Some(Self::Import),
            "_module_file" => Some(Self::ModuleFile),
            "_local_area" => Some(Self::LocalArea),
            "_extract" => Some(Self::Extract),
            "_args" => Some(Self::Args),
            "_run" => Some(Self::Run),
            "_escape_bin" => Some(Self::EscapeBin),
            "_unit" => Some(Self::Unit),
            "_get_area" => Some(Self::GetArea),
            "_download" => Some(Self::Download),
            "_throw" => Some(Self::Throw),
            "_read_file" => Some(Self::ReadFile),
            "_sha256" => Some(Self::Sha256),
            "_bytes_to_string" => Some(Self::BytesToString),
            "_parse_toml" => Some(Self::ParseToml),
            _ => None,
        }
    }

    pub fn cache_strategy(&self) -> CacheStrategy {
        match self {
            // These are very cheap shouldn't bother caching
            Self::Throw | Self::Unit | Self::GetFile => CacheStrategy::Never,
            _ => CacheStrategy::Always,
        }
    }

    pub fn call_internal_function(
        self,
        driver: &dyn DriverTrait,
        rir: &mut Rir,
        cx: &mut Cx,
        nid: NodeId,
        fn_call: &FnCall,
        arg_values: Vec<(NodeId, Value)>,
    ) -> ResultValue {
        let icx = InternalCx {
            driver,
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
            Self::Throw => throw(icx),
            Self::ReadFile => read_file(icx),
            Self::Sha256 => sha256(icx),
            Self::BytesToString => bytes_to_string(icx),
            Self::ParseToml => parse_toml(icx),
        }
    }
}

struct InternalCx<'a, 'b> {
    driver: &'a dyn DriverTrait,
    rir: &'a mut Rir,
    cx: &'a mut Cx<'b>,
    nid: NodeId,
    fn_call: &'a FnCall,
    arg_values: Vec<(NodeId, Value)>,
}

impl InternalCx<'_, '_> {
    fn single_arg<'c, T: ValueInner>(&'c self, rain_type: &'static [RainTypeId]) -> Result<&'c T> {
        match &self.arg_values[..] {
            [(arg_nid, arg_value)] => {
                let arg: &T = arg_value
                    .downcast_ref_error::<T>(rain_type)
                    .map_err(|err| self.cx.nid_err(*arg_nid, err))?;
                Ok(arg)
            }
            _ => Err(self.cx.err(
                self.fn_call.rparen_token,
                RunnerError::IncorrectArgs {
                    required: 1..=1,
                    actual: self.arg_values.len(),
                },
            )),
        }
    }

    fn incorrect_args(self, required: RangeInclusive<usize>) -> ResultValue {
        Err(self.cx.err(
            self.fn_call.rparen_token,
            RunnerError::IncorrectArgs {
                required,
                actual: self.arg_values.len(),
            },
        ))
    }
}

fn print_implementation(icx: InternalCx) -> ResultValue {
    let args: Vec<String> = icx
        .arg_values
        .into_iter()
        .map(|(_, a)| format!("{a}"))
        .collect();
    icx.driver.print(args.join(" "));
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
            if !icx.driver.exists(&file).map_err(|err| {
                icx.cx
                    .nid_err(*relative_path_nid, RunnerError::AreaIOError(err))
            })? {
                return Err(icx
                    .cx
                    .nid_err(*relative_path_nid, RunnerError::FileDoesNotExist));
            }
            Ok(Value::new(file))
        }
        [
            (area_nid, area_value),
            (absolute_path_nid, absolute_path_value),
        ] => {
            let area: &FileArea = area_value
                .downcast_ref_error(&[RainTypeId::FileArea])
                .map_err(|err| icx.cx.nid_err(*area_nid, err))?;
            let absolute_path: &String = absolute_path_value
                .downcast_ref_error(&[RainTypeId::String])
                .map_err(|err| icx.cx.nid_err(*absolute_path_nid, err))?;
            let file = File::new_checked(area.clone(), absolute_path)
                .map_err(|err| icx.cx.nid_err(icx.nid, err.into()))?;
            if !icx.driver.exists(&file).map_err(|err| {
                icx.cx
                    .nid_err(*absolute_path_nid, RunnerError::AreaIOError(err))
            })? {
                return Err(icx
                    .cx
                    .nid_err(*absolute_path_nid, RunnerError::FileDoesNotExist));
            }
            Ok(Value::new(file))
        }
        _ => icx.incorrect_args(1..=2),
    }
}

fn import_implementation(icx: InternalCx) -> ResultValue {
    match &icx.arg_values[..] {
        [(file_nid, file_value)] => {
            let file: &File = file_value
                .downcast_ref_error(&[RainTypeId::File])
                .map_err(|err| icx.cx.nid_err(*file_nid, err))?;
            let resolved_path = icx.driver.resolve_file(file);
            let src = std::fs::read_to_string(&resolved_path)
                .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::ImportIOError(err)))?;
            let module = crate::ast::parser::parse_module(&src);
            let id = icx
                .rir
                .insert_module(file.clone(), src, module)
                .map_err(ErrorSpan::convert)?;
            Ok(Value::new(Module { id }))
        }
        _ => icx.incorrect_args(1..=1),
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
                .driver
                .extract(file)
                .map_err(|err| icx.cx.nid_err(icx.nid, err))?;
            Ok(Value::new(area))
        }
        _ => icx.incorrect_args(1..=1),
    }
}

fn args_implementation(_icx: InternalCx) -> ResultValue {
    let args: Vec<_> = std::env::args().skip(1).map(Value::new).collect();
    Ok(Value::new(RainList(args)))
}

fn run_implementation(icx: InternalCx) -> ResultValue {
    match &icx.arg_values[..] {
        [(area_nid, area_value), (file_nid, file_value), args @ ..] => {
            let overlay_area = match area_value.rain_type_id() {
                RainTypeId::Unit => None,
                RainTypeId::FileArea => {
                    let area: &FileArea = area_value
                        .downcast_ref_error(&[RainTypeId::FileArea])
                        .map_err(|err| icx.cx.nid_err(*area_nid, err))?;
                    Some(area)
                }
                _ => Err(icx.cx.nid_err(*area_nid, RunnerError::GenericRunError))?,
            };
            let file: &File = file_value
                .downcast_ref_error(&[RainTypeId::File])
                .map_err(|err| icx.cx.nid_err(*file_nid, err))?;
            let args = args
                .iter()
                .map(|(nid, value)| match value.rain_type_id() {
                    RainTypeId::String => Ok(value
                        .downcast_ref_error::<String>(&[RainTypeId::String])
                        .map_err(|err| icx.cx.nid_err(*nid, err))?
                        .to_string()),
                    RainTypeId::File => Ok(icx
                        .driver
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
                .collect::<Result<Vec<String>>>()?;
            let status = icx
                .driver
                .run(overlay_area, file, args)
                .map_err(|err| icx.cx.nid_err(icx.nid, err))?;
            let mut m = HashMap::new();
            m.insert("success".to_owned(), Value::new(status.success));
            m.insert(
                "exit_code".to_owned(),
                Value::new(RainInteger(status.exit_code.unwrap_or(-1).into())),
            );
            m.insert("area".to_owned(), Value::new(status.area));
            m.insert("stdout".to_owned(), Value::new(status.stdout));
            m.insert("stderr".to_owned(), Value::new(status.stderr));
            Ok(Value::new(RainRecord(m)))
        }
        _ => icx.incorrect_args(1..=2),
    }
}

fn escape_bin(icx: InternalCx) -> ResultValue {
    match &icx.arg_values[..] {
        [(name_nid, name_value)] => {
            let name: &String = name_value
                .downcast_ref_error(&[RainTypeId::String])
                .map_err(|err| icx.cx.nid_err(*name_nid, err))?;
            let path = icx
                .driver
                .escape_bin(name)
                .ok_or_else(|| icx.cx.nid_err(icx.nid, RunnerError::GenericRunError))?;
            let f = File::new_checked(FileArea::Escape, path.to_string_lossy().as_ref())
                .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::PathError(err)))?;
            Ok(Value::new(f))
        }
        _ => icx.incorrect_args(1..=1),
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
        _ => icx.incorrect_args(1..=1),
    }
}

fn download(icx: InternalCx) -> ResultValue {
    match &icx.arg_values[..] {
        [(url_nid, url_value), (name_nid, name_value)] => {
            let url: &String = url_value
                .downcast_ref_error(&[RainTypeId::String])
                .map_err(|err| icx.cx.nid_err(*url_nid, err))?;
            let name: &String = name_value
                .downcast_ref_error(&[RainTypeId::String])
                .map_err(|err| icx.cx.nid_err(*name_nid, err))?;
            let DownloadStatus {
                ok,
                status_code,
                file,
            } = icx
                .driver
                .download(url, name)
                .map_err(|err| icx.cx.nid_err(icx.nid, err))?;
            let mut m = HashMap::new();
            m.insert("ok".to_owned(), Value::new(ok));
            m.insert(
                "status_code".to_owned(),
                Value::new(RainInteger(status_code.unwrap_or_default().into())),
            );
            if let Some(file) = file {
                m.insert("file".to_owned(), Value::new(file));
            } else {
                m.insert("file".to_owned(), Value::new(RainUnit));
            }
            Ok(Value::new(RainRecord(m)))
        }
        _ => icx.incorrect_args(2..=2),
    }
}

fn throw(icx: InternalCx) -> ResultValue {
    match &icx.arg_values[..] {
        [(_, err_value)] => Err(icx
            .cx
            .module
            .span(icx.nid)
            .with_module(icx.cx.module.id)
            .with_error(super::error::Throwing::Recoverable(err_value.clone()))),
        _ => icx.incorrect_args(1..=1),
    }
}

fn read_file(icx: InternalCx) -> ResultValue {
    match &icx.arg_values[..] {
        [(file_nid, file_value)] => {
            let file: &File = file_value
                .downcast_ref_error(&[RainTypeId::File])
                .map_err(|err| icx.cx.nid_err(*file_nid, err))?;
            Ok(Value::new(
                icx.driver
                    .read_file(file)
                    .map_err(|err| icx.cx.nid_err(icx.nid, err))?,
            ))
        }
        _ => icx.incorrect_args(1..=1),
    }
}

fn sha256(icx: InternalCx) -> ResultValue {
    match &icx.arg_values[..] {
        [(file_nid, file_value)] => {
            let file: &File = file_value
                .downcast_ref_error(&[RainTypeId::File])
                .map_err(|err| icx.cx.nid_err(*file_nid, err))?;
            Ok(Value::new(
                icx.driver
                    .sha256(file)
                    .map_err(|err| icx.cx.nid_err(icx.nid, err))?,
            ))
        }
        _ => icx.incorrect_args(1..=1),
    }
}

#[expect(clippy::unwrap_used)]
fn bytes_to_string(icx: InternalCx) -> ResultValue {
    match &icx.arg_values[..] {
        [(bytes_nid, bytes_value)] => {
            let bytes: &RainList = bytes_value
                .downcast_ref_error(&[RainTypeId::List])
                .map_err(|err| icx.cx.nid_err(*bytes_nid, err))?;
            let bytes: Vec<u8> = bytes
                .0
                .iter()
                .map(|b| -> u8 {
                    b.downcast_ref::<RainInteger>()
                        .unwrap()
                        .0
                        .iter_u32_digits()
                        .next()
                        .unwrap()
                        .try_into()
                        .unwrap()
                })
                .collect();
            Ok(Value::new(String::from_utf8(bytes).unwrap()))
        }
        _ => icx.incorrect_args(1..=1),
    }
}

fn parse_toml(icx: InternalCx) -> ResultValue {
    fn toml_to_rain(v: toml::Value) -> Value {
        match v {
            toml::Value::String(s) => Value::new(s),
            toml::Value::Integer(n) => Value::new(RainInteger(BigInt::from(n))),
            toml::Value::Float(f) => Value::new(f.to_string()),
            toml::Value::Boolean(b) => Value::new(b),
            toml::Value::Datetime(datetime) => Value::new(datetime.to_string()),
            toml::Value::Array(vec) => {
                Value::new(RainList(vec.into_iter().map(toml_to_rain).collect()))
            }
            toml::Value::Table(map) => Value::new(RainRecord(
                map.into_iter()
                    .map(|(k, v)| (k.replace('-', "_"), toml_to_rain(v)))
                    .collect(),
            )),
        }
    }

    let contents = icx.single_arg::<String>(&[RainTypeId::String])?;
    let parsed: toml::Value = toml::de::from_str(contents).map_err(|err| {
        icx.cx.nid_err(
            icx.nid,
            RunnerError::Makeshift(err.message().to_owned().into()),
        )
    })?;
    Ok(toml_to_rain(parsed))
}
