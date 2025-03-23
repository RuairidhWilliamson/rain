#![allow(clippy::unnecessary_wraps, clippy::needless_pass_by_value)]

use std::{collections::HashMap, ops::RangeInclusive};

use indexmap::IndexMap;
use num_bigint::BigInt;

use crate::{
    afs::{absolute::AbsolutePathBuf, area::FileArea, error::PathError, file::File},
    ast::{FnCall, NodeId},
    driver::{DownloadStatus, DriverTrait, RunOptions},
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
    Debug,
    GetFile,
    Import,
    ModuleFile,
    LocalArea,
    ExtractZip,
    ExtractTarGz,
    ExtractTarXz,
    Args,
    Run,
    EscapeBin,
    Unit,
    GetArea,
    Download,
    Throw,
    Sha256,
    BytesToString,
    ParseToml,
    MergeDirs,
    ReadFile,
    WriteFile,
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
            "_debug" => Some(Self::Debug),
            "_get_file" => Some(Self::GetFile),
            "_import" => Some(Self::Import),
            "_module_file" => Some(Self::ModuleFile),
            "_local_area" => Some(Self::LocalArea),
            "_extract_zip" => Some(Self::ExtractZip),
            "_extract_tar_gz" => Some(Self::ExtractTarGz),
            "_extract_tar_xz" => Some(Self::ExtractTarXz),
            "_args" => Some(Self::Args),
            "_run" => Some(Self::Run),
            "_escape_bin" => Some(Self::EscapeBin),
            "_unit" => Some(Self::Unit),
            "_get_area" => Some(Self::GetArea),
            "_download" => Some(Self::Download),
            "_throw" => Some(Self::Throw),
            "_sha256" => Some(Self::Sha256),
            "_bytes_to_string" => Some(Self::BytesToString),
            "_parse_toml" => Some(Self::ParseToml),
            "_merge_dirs" => Some(Self::MergeDirs),
            "_read_file" => Some(Self::ReadFile),
            "_write_file" => Some(Self::WriteFile),
            _ => None,
        }
    }

    pub fn cache_strategy(&self) -> CacheStrategy {
        CacheStrategy::Never
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
            Self::Print => print(icx),
            Self::GetFile => get_file(icx),
            Self::Import => import(icx),
            Self::ModuleFile => module_file(icx),
            Self::LocalArea => local_area(icx),
            Self::ExtractZip => extract_zip(icx),
            Self::ExtractTarGz => extract_tar_gz(icx),
            Self::ExtractTarXz => extract_tar_xz(icx),
            Self::Args => args_implementation(icx),
            Self::Run => run_implementation(icx),
            Self::EscapeBin => escape_bin(icx),
            Self::Unit => unit(icx),
            Self::GetArea => get_area(icx),
            Self::Download => download(icx),
            Self::Throw => throw(icx),
            Self::Sha256 => sha256(icx),
            Self::BytesToString => bytes_to_string(icx),
            Self::ParseToml => parse_toml(icx),
            Self::MergeDirs => merge_dirs(icx),
            Self::ReadFile => read_file(icx),
            Self::WriteFile => write_file(icx),
            Self::Debug => debug(icx),
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

    fn no_args(&self) -> Result<()> {
        if self.arg_values.is_empty() {
            Ok(())
        } else {
            Err(self.cx.err(
                self.fn_call.rparen_token,
                RunnerError::IncorrectArgs {
                    required: 0..=0,
                    actual: self.arg_values.len(),
                },
            ))
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

fn print(icx: InternalCx) -> ResultValue {
    let args: Vec<String> = icx
        .arg_values
        .into_iter()
        .map(|(_, a)| {
            if let Some(s) = a.downcast_ref::<String>() {
                s.to_owned()
            } else {
                format!("{a}")
            }
        })
        .collect();
    icx.driver.print(args.join(" "));
    Ok(Value::new(RainUnit))
}

fn get_file(icx: InternalCx) -> ResultValue {
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
                    .nid_err(*relative_path_nid, RunnerError::FileDoesNotExist(file)));
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
                    .nid_err(*absolute_path_nid, RunnerError::FileDoesNotExist(file)));
            }
            Ok(Value::new(file))
        }
        _ => icx.incorrect_args(1..=2),
    }
}

fn import(icx: InternalCx) -> ResultValue {
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

fn module_file(icx: InternalCx) -> ResultValue {
    icx.no_args()?;
    Ok(Value::new(icx.cx.module.file.clone()))
}

fn local_area(icx: InternalCx) -> ResultValue {
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

fn extract_zip(icx: InternalCx) -> ResultValue {
    let file = icx.single_arg::<File>(&[RainTypeId::File])?;
    let area = icx
        .driver
        .extract_zip(file)
        .map_err(|err| icx.cx.nid_err(icx.nid, err))?;
    Ok(Value::new(area))
}

fn extract_tar_gz(icx: InternalCx) -> ResultValue {
    let file = icx.single_arg::<File>(&[RainTypeId::File])?;
    let area = icx
        .driver
        .extract_tar_gz(file)
        .map_err(|err| icx.cx.nid_err(icx.nid, err))?;
    Ok(Value::new(area))
}

fn extract_tar_xz(icx: InternalCx) -> ResultValue {
    let file = icx.single_arg::<File>(&[RainTypeId::File])?;
    let area = icx
        .driver
        .extract_tar_xz(file)
        .map_err(|err| icx.cx.nid_err(icx.nid, err))?;
    Ok(Value::new(area))
}

fn args_implementation(_icx: InternalCx) -> ResultValue {
    let args: Vec<_> = std::env::args().skip(1).map(Value::new).collect();
    Ok(Value::new(RainList(args)))
}

fn run_implementation(icx: InternalCx) -> ResultValue {
    match &icx.arg_values[..] {
        [
            (area_nid, area_value),
            (file_nid, file_value),
            (args_nid, args_value),
            (env_nid, env_value),
        ] => {
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
            let args: &RainList = args_value
                .downcast_ref_error(&[RainTypeId::List])
                .map_err(|err| icx.cx.nid_err(*args_nid, err))?;
            let args = args
                .0
                .iter()
                .map(|value| stringify_args(&icx, *args_nid, value))
                .collect::<Result<Vec<String>>>()?;
            let env: &RainRecord = env_value
                .downcast_ref_error(&[RainTypeId::List])
                .map_err(|err| icx.cx.nid_err(*env_nid, err))?;
            let env = env
                .0
                .iter()
                .map(|(key, value)| stringify_env(&icx, *env_nid, key, value))
                .collect::<Result<HashMap<String, String>>>()?;
            let status = icx
                .driver
                .run(
                    overlay_area,
                    file,
                    args,
                    RunOptions {
                        inherit_env: false,
                        env,
                    },
                )
                .map_err(|err| icx.cx.nid_err(icx.nid, err))?;
            let mut m = IndexMap::new();
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
        _ => icx.incorrect_args(4..=4),
    }
}

fn stringify_env(
    icx: &InternalCx<'_, '_>,
    env_nid: NodeId,
    key: &String,
    value: &Value,
) -> Result<(String, String)> {
    match value.rain_type_id() {
        RainTypeId::String => Ok((
            key.to_owned(),
            value
                .downcast_ref_error::<String>(&[RainTypeId::String])
                .map_err(|err| icx.cx.nid_err(env_nid, err))?
                .to_string(),
        )),
        RainTypeId::File => Ok((
            key.to_owned(),
            icx.driver
                .resolve_file(
                    value
                        .downcast_ref_error::<File>(&[RainTypeId::File])
                        .map_err(|err| icx.cx.nid_err(env_nid, err))?,
                )
                .display()
                .to_string(),
        )),
        type_id => Err(icx.cx.nid_err(
            env_nid,
            RunnerError::ExpectedType {
                actual: type_id,
                expected: &[RainTypeId::String, RainTypeId::File],
            },
        )),
    }
}

fn stringify_args(icx: &InternalCx<'_, '_>, args_nid: NodeId, value: &Value) -> Result<String> {
    match value.rain_type_id() {
        RainTypeId::String => Ok(value
            .downcast_ref_error::<String>(&[RainTypeId::String])
            .map_err(|err| icx.cx.nid_err(args_nid, err))?
            .to_string()),
        RainTypeId::File => Ok(icx
            .driver
            .resolve_file(
                value
                    .downcast_ref_error::<File>(&[RainTypeId::File])
                    .map_err(|err| icx.cx.nid_err(args_nid, err))?,
            )
            .display()
            .to_string()),
        type_id => Err(icx.cx.nid_err(
            args_nid,
            RunnerError::ExpectedType {
                actual: type_id,
                expected: &[RainTypeId::String, RainTypeId::File],
            },
        )),
    }
}

fn escape_bin(icx: InternalCx) -> ResultValue {
    let name: &String = icx.single_arg(&[RainTypeId::String])?;
    let path = icx
        .driver
        .escape_bin(name)
        .ok_or_else(|| icx.cx.nid_err(icx.nid, RunnerError::GenericRunError))?;
    let f = File::new_checked(FileArea::Escape, path.to_string_lossy().as_ref())
        .map_err(|err| icx.cx.nid_err(icx.nid, RunnerError::PathError(err)))?;
    Ok(Value::new(f))
}

fn unit(icx: InternalCx) -> ResultValue {
    icx.no_args()?;
    Ok(Value::new(RainUnit))
}

fn get_area(icx: InternalCx) -> ResultValue {
    let file: &File = icx.single_arg(&[RainTypeId::File])?;
    Ok(Value::new(file.area.clone()))
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
            let mut m = IndexMap::new();
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
    let bytes: &RainList = icx.single_arg(&[RainTypeId::List])?;
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

fn merge_dirs(icx: InternalCx) -> ResultValue {
    let dirs: &RainList = icx.single_arg(&[RainTypeId::List])?;
    let dirs = dirs
        .0
        .iter()
        .map(|area| area.downcast_ref_error::<File>(&[RainTypeId::File]))
        .collect::<Result<Vec<&File>, RunnerError>>()
        .map_err(|err| icx.cx.nid_err(icx.nid, err))?;
    let merged_area = icx
        .driver
        .merge_dirs(&dirs)
        .map_err(|err| icx.cx.nid_err(icx.nid, err))?;
    Ok(Value::new(merged_area))
}

fn read_file(icx: InternalCx) -> ResultValue {
    let file: &File = icx.single_arg(&[RainTypeId::File])?;
    Ok(Value::new(
        icx.driver
            .read_file(file)
            .map_err(|err| icx.cx.nid_err(icx.nid, err))?,
    ))
}

fn write_file(icx: InternalCx) -> ResultValue {
    match &icx.arg_values[..] {
        [(contents_nid, contents_value), (name_nid, name_value)] => {
            let contents: &String = contents_value
                .downcast_ref_error::<String>(&[RainTypeId::String])
                .map_err(|err| icx.cx.nid_err(*contents_nid, err))?;
            let name: &String = name_value
                .downcast_ref_error::<String>(&[RainTypeId::String])
                .map_err(|err| icx.cx.nid_err(*name_nid, err))?;
            Ok(Value::new(
                icx.driver
                    .write_file(contents, name)
                    .map_err(|err| icx.cx.nid_err(icx.nid, err))?,
            ))
        }
        _ => Err(icx.cx.err(
            icx.fn_call.rparen_token,
            RunnerError::IncorrectArgs {
                required: 2..=2,
                actual: icx.arg_values.len(),
            },
        )),
    }
}

fn debug(mut icx: InternalCx) -> ResultValue {
    if icx.arg_values.len() != 1 {
        return icx.incorrect_args(1..=1);
    }
    let Some((_nid, value)) = icx.arg_values.pop() else {
        return icx.incorrect_args(1..=1);
    };
    let p = if let Some(s) = value.downcast_ref::<String>() {
        s.to_owned()
    } else {
        format!("{value}")
    };
    icx.driver.print(p);
    Ok(value)
}
