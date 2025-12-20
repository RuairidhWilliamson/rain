#![allow(clippy::unnecessary_wraps)]

mod download;
mod run;

use std::{ops::RangeInclusive, path::Path, str::FromStr as _, sync::Arc, time::Instant};

use indexmap::IndexMap;
use num_bigint::BigInt;

use crate::{
    afs::{
        absolute::AbsolutePathBuf,
        area::FileArea,
        dir::Dir,
        entry::{FSEntry, FSEntryTrait as _},
        error::PathError,
        file::File,
        path::SealedFilePath,
    },
    ast::NodeId,
    driver::{DriverTrait, FSEntryQueryResult},
    local_span::LocalSpan,
    runner::cache::CacheTrait,
};

use super::{
    Result, ResultValue,
    cache::{CacheEntry, CacheKey},
    cx::Cx,
    dep::Dep,
    error::{RunnerError, Throwing},
    value::{RainInteger, RainList, RainRecord, RainTypeId, Value},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum InternalFunction {
    Print,
    Debug,
    GetFile,
    GetDir,
    Import,
    ModuleFile,
    ExtractZip,
    ExtractTarGz,
    ExtractTarXz,
    ExtractTar,
    Run,
    EscapeBin,
    Unit,
    GetArea,
    Download,
    Throw,
    Sha256,
    Sha512,
    BytesToString,
    ParseToml,
    ParseJSON,
    CreateArea,
    ReadFile,
    CreateFile,
    LocalArea,
    SplitString,
    Index,
    HostInfo,
    StringContains,
    ExportToLocal,
    CheckExportToLocal,
    FileMetadata,
    Glob,
    Foreach,
    Stringify,
    EscapeRun,
    Prelude,
    CreateTar,
    CreateTarGz,
    RustEq,
    GetSecret,
    SetCacheNever,
    ClearCallingCacheDeps,
    MergeRecords,
    ParseTargetTriple,
    GitContents,
    GitLfsSmudge,
    EnvVar,
    CopyFile,
    ListLength,
    EscapeHard,
    ChrootRun,
    Flatten,
    GetType,
}

impl std::fmt::Display for InternalFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

impl InternalFunction {
    pub fn evaluate_internal_function_name(name: &str) -> Option<Self> {
        match name {
            "_print" => Some(Self::Print),
            "_debug" => Some(Self::Debug),
            "_get_file" => Some(Self::GetFile),
            "_get_dir" => Some(Self::GetDir),
            "_import" => Some(Self::Import),
            "_module_file" => Some(Self::ModuleFile),
            "_extract_zip" => Some(Self::ExtractZip),
            "_extract_tar_gz" => Some(Self::ExtractTarGz),
            "_extract_tar_xz" => Some(Self::ExtractTarXz),
            "_extract_tar" => Some(Self::ExtractTar),
            "_run" => Some(Self::Run),
            "_escape_bin" => Some(Self::EscapeBin),
            "_unit" => Some(Self::Unit),
            "_get_area" => Some(Self::GetArea),
            "_download" => Some(Self::Download),
            "_throw" => Some(Self::Throw),
            "_sha256" => Some(Self::Sha256),
            "_sha512" => Some(Self::Sha512),
            "_bytes_to_string" => Some(Self::BytesToString),
            "_parse_toml" => Some(Self::ParseToml),
            "_create_area" => Some(Self::CreateArea),
            "_read_file" => Some(Self::ReadFile),
            "_create_file" => Some(Self::CreateFile),
            "_local_area" => Some(Self::LocalArea),
            "_split_string" => Some(Self::SplitString),
            "_index" => Some(Self::Index),
            "_host_info" => Some(Self::HostInfo),
            "_string_contains" => Some(Self::StringContains),
            "_export_to_local" => Some(Self::ExportToLocal),
            "_check_export_to_local" => Some(Self::CheckExportToLocal),
            "_file_metadata" => Some(Self::FileMetadata),
            "_glob" => Some(Self::Glob),
            "_foreach" => Some(Self::Foreach),
            "_stringify" => Some(Self::Stringify),
            "_escape_run" => Some(Self::EscapeRun),
            "_prelude" => Some(Self::Prelude),
            "_create_tar" => Some(Self::CreateTar),
            "_rust_eq" => Some(Self::RustEq),
            "_get_secret" => Some(Self::GetSecret),
            "_set_cache_never" => Some(Self::SetCacheNever),
            "_clear_calling_cache_deps" => Some(Self::ClearCallingCacheDeps),
            "_merge_records" => Some(Self::MergeRecords),
            "_parse_target_triple" => Some(Self::ParseTargetTriple),
            "_git_contents" => Some(Self::GitContents),
            "_git_lfs_smudge" => Some(Self::GitLfsSmudge),
            "_env_var" => Some(Self::EnvVar),
            "_copy_file" => Some(Self::CopyFile),
            "_list_length" => Some(Self::ListLength),
            "_escape_hard" => Some(Self::EscapeHard),
            "_create_tar_gz" => Some(Self::CreateTarGz),
            "_flatten" => Some(Self::Flatten),
            "_parse_json" => Some(Self::ParseJSON),
            "_get_type" => Some(Self::GetType),
            _ => None,
        }
    }

    pub fn call_internal_function<Driver: DriverTrait, Cache: CacheTrait>(
        self,
        icx: InternalCx<Driver, Cache>,
    ) -> ResultValue {
        match self {
            Self::Print => icx.print(),
            Self::Debug => icx.debug(),
            Self::GetFile => icx.get_file(),
            Self::GetDir => icx.get_dir(),
            Self::Import => icx.import(),
            Self::ModuleFile => icx.module_file(),
            Self::ExtractZip => icx.extract_zip(),
            Self::ExtractTarGz => icx.extract_tar_gz(),
            Self::ExtractTarXz => icx.extract_tar_xz(),
            Self::ExtractTar => icx.extract_tar(),
            Self::Run => icx.run(),
            Self::EscapeBin => icx.escape_bin(),
            Self::Unit => icx.unit(),
            Self::GetArea => icx.get_area(),
            Self::Download => icx.download(),
            Self::Throw => icx.throw(),
            Self::Sha256 => icx.sha256(),
            Self::Sha512 => icx.sha512(),
            Self::BytesToString => icx.bytes_to_string(),
            Self::ParseToml => icx.parse_toml(),
            Self::CreateArea => icx.create_area(),
            Self::ReadFile => icx.read_file(),
            Self::CreateFile => icx.create_file(),
            Self::LocalArea => icx.local_area(),
            Self::SplitString => icx.split_string(),
            Self::Index => icx.index(),
            Self::HostInfo => icx.host_info(),
            Self::StringContains => icx.string_contains(),
            Self::ExportToLocal => icx.export_to_local(),
            Self::CheckExportToLocal => icx.check_export_to_local(),
            Self::FileMetadata => icx.file_metadata(),
            Self::Glob => icx.glob(),
            Self::Foreach => icx.foreach(),
            Self::Stringify => icx.stringify(),
            Self::EscapeRun => icx.escape_run(),
            Self::Prelude => icx.prelude(),
            Self::CreateTar => icx.create_tar(),
            Self::RustEq => icx.rust_eq(),
            Self::GetSecret => icx.get_secret(),
            Self::SetCacheNever => icx.set_cache_never(),
            Self::ClearCallingCacheDeps => icx.clear_calling_cache_deps(),
            Self::MergeRecords => icx.merge_records(),
            Self::ParseTargetTriple => icx.parse_target_triple(),
            Self::GitContents => icx.git_contents(),
            Self::GitLfsSmudge => icx.git_lfs_smudge(),
            Self::EnvVar => icx.env_var(),
            Self::CopyFile => icx.copy_file(),
            Self::ListLength => icx.list_length(),
            Self::EscapeHard => icx.escape_hard(),
            Self::ChrootRun => icx.chroot_run(),
            Self::CreateTarGz => icx.create_tar_gz(),
            Self::Flatten => icx.flatten(),
            Self::ParseJSON => icx.parse_json(),
            Self::GetType => icx.get_type(),
        }
    }
}

macro_rules! single_arg {
    ($icx:ident) => {
        match &$icx.arg_values[..] {
            [(arg_nid, arg_value)] => (*arg_nid, arg_value),
            _ => {
                return Err($icx.cx.err(
                    $icx.call_span,
                    RunnerError::IncorrectArgs {
                        required: 1..=1,
                        actual: $icx.arg_values.len(),
                    },
                ))
            }
        }
    };
}

macro_rules! two_args {
    ($icx:ident) => {
        match &$icx.arg_values[..] {
            [(arg1_nid, arg1_value), (arg2_nid, arg2_value)] => {
                ((*arg1_nid, arg1_value), (*arg2_nid, arg2_value))
            }
            _ => {
                return Err($icx.cx.err(
                    $icx.call_span,
                    RunnerError::IncorrectArgs {
                        required: 1..=1,
                        actual: $icx.arg_values.len(),
                    },
                ))
            }
        }
    };
}

macro_rules! expect_type {
    ($icx:expr, $typ:ident, $nid_value:expr) => {{
        let (nid, value) = $nid_value;
        let Value::$typ(v) = value else {
            return Err($icx.cx.nid_err(
                nid,
                RunnerError::ExpectedType {
                    actual: value.rain_type_id(),
                    expected: &[RainTypeId::$typ],
                },
            ));
        };
        debug_assert_eq!(value.rain_type_id(), RainTypeId::$typ);
        v
    }};
}

struct Call<'a> {
    driver: &'a dyn DriverTrait,
    s: String,
}

impl Drop for Call<'_> {
    fn drop(&mut self) {
        self.driver.exit_call(&self.s);
    }
}

#[must_use]
fn enter_call(driver: &dyn DriverTrait, s: String) -> Call<'_> {
    driver.enter_call(&s);
    Call { driver, s }
}

// TODO: Cleanup all those lifetimes :o
pub struct InternalCx<'a, 'b, 'c, 'd, 'e, Driver, Cache> {
    pub func: InternalFunction,
    pub runner: &'a mut super::Runner<'d, Driver, Cache>,
    pub cx: &'c mut Cx<'b>,
    pub nid: NodeId,
    pub call_span: LocalSpan,
    pub arg_values: Vec<(NodeId, Value)>,
    pub deps: &'e mut Vec<Dep>,
    /// Set to false to hint to the caller that this is probably less efficient to go to cache
    pub cache_hint: &'e mut bool,
}

impl<Driver: DriverTrait, Cache: CacheTrait> InternalCx<'_, '_, '_, '_, '_, Driver, Cache> {
    fn no_args(&self) -> Result<()> {
        if self.arg_values.is_empty() {
            Ok(())
        } else {
            Err(self.cx.err(
                self.call_span,
                RunnerError::IncorrectArgs {
                    required: 0..=0,
                    actual: self.arg_values.len(),
                },
            ))
        }
    }

    fn incorrect_args<T>(&self, required: RangeInclusive<usize>) -> Result<T> {
        Err(self.cx.err(
            self.call_span,
            RunnerError::IncorrectArgs {
                required,
                actual: self.arg_values.len(),
            },
        ))
    }

    fn expect_dir_or_area(&self, arg_nid: NodeId, arg_value: &Value) -> Result<Arc<Dir>> {
        match arg_value {
            Value::FileArea(file_area) => Ok(Arc::new(Dir::root(file_area.as_ref().clone()))),
            Value::Dir(dir) => Ok(Arc::clone(dir)),
            _ => Err(self.cx.nid_err(
                arg_nid,
                RunnerError::ExpectedType {
                    actual: arg_value.rain_type_id(),
                    expected: &[RainTypeId::Dir, RainTypeId::FileArea],
                },
            )),
        }
    }

    fn check_escape_mode(&self) -> Result<()> {
        if self.runner.seal {
            Err(self.cx.nid_err(self.nid, RunnerError::CantEscapeSeal))
        } else {
            Ok(())
        }
    }

    fn print(self) -> ResultValue {
        self.deps.push(Dep::Print);
        let args: Vec<String> = self
            .arg_values
            .into_iter()
            .map(|(_, a)| {
                if let Value::String(s) = a {
                    s.as_ref().clone()
                } else {
                    format!("{a}")
                }
            })
            .collect();
        self.runner.driver.print(args.join(" "));
        Ok(Value::Unit)
    }

    fn file_area_resolve_path(&mut self) -> Result<FSEntry> {
        match &self.arg_values[..] {
            [(relative_path_nid, relative_path_value)] => {
                let relative_path =
                    expect_type!(self, String, (*relative_path_nid, relative_path_value));
                let file = self
                    .cx
                    .module
                    .file()
                    .map_err(|err| self.cx.nid_err(self.nid, err))?;
                self.cx.add_dep_file_area(file.area());
                let file_path = file
                    .path()
                    .parent()
                    .ok_or_else(|| {
                        self.cx
                            .nid_err(self.nid, PathError::NoParentDirectory.into())
                    })?
                    .join(relative_path.as_str())
                    .map_err(|err| self.cx.nid_err(*relative_path_nid, err.into()))?;
                Ok(FSEntry {
                    area: file.area().clone(),
                    path: file_path,
                })
            }
            [(parent_nid, parent_value), (path_nid, path_value)] => {
                let path = expect_type!(self, String, (path_nid, path_value));
                match parent_value {
                    Value::FileArea(area) => {
                        self.cx.add_dep_file_area(area);
                        let file_path = SealedFilePath::new(path)
                            .map_err(|err| self.cx.nid_err(*path_nid, err.into()))?;
                        Ok(FSEntry {
                            area: area.as_ref().clone(),
                            path: file_path,
                        })
                    }
                    Value::Dir(dir) => {
                        let area = dir.area();
                        self.cx.add_dep_file_area(area);
                        let base_path = dir.path();
                        let path = base_path
                            .join(path)
                            .map_err(|err| self.cx.nid_err(*path_nid, err.into()))?;
                        Ok(FSEntry {
                            area: area.clone(),
                            path,
                        })
                    }
                    _ => Err(self.cx.nid_err(
                        *parent_nid,
                        RunnerError::ExpectedType {
                            actual: parent_value.rain_type_id(),
                            expected: &[RainTypeId::FileArea, RainTypeId::Dir],
                        },
                    )),
                }
            }
            _ => self.incorrect_args(1..=2),
        }
    }

    fn get_file(mut self) -> ResultValue {
        let entry = self.file_area_resolve_path()?;
        match self
            .runner
            .driver
            .query_fs(&entry)
            .map_err(|err| self.cx.nid_err(self.nid, RunnerError::AreaIOError(err)))?
        {
            FSEntryQueryResult::File => {
                // Safety: Checked that the file exists and is a file
                let file = unsafe { File::new(entry) };
                Ok(Value::File(Arc::new(file)))
            }
            result => Err(self
                .cx
                .nid_err(self.nid, RunnerError::FSQuery(entry, result))),
        }
    }

    fn get_dir(mut self) -> ResultValue {
        let entry = self.file_area_resolve_path()?;
        match self
            .runner
            .driver
            .query_fs(&entry)
            .map_err(|err| self.cx.nid_err(self.nid, RunnerError::AreaIOError(err)))?
        {
            FSEntryQueryResult::Directory => {
                // Safety: Checked that the dir exists and is a dir
                let dir = unsafe { Dir::new(entry) };
                Ok(Value::Dir(Arc::new(dir)))
            }
            result => Err(self
                .cx
                .nid_err(self.nid, RunnerError::FSQuery(entry, result))),
        }
    }

    fn import(self) -> ResultValue {
        let f = expect_type!(self, File, single_arg!(self));
        let cache_key = CacheKey::Import {
            file: Arc::clone(f),
        };
        if let Some(v) = self.runner.cache.get_value(&cache_key) {
            return Ok(v);
        }
        let start = Instant::now();
        let src = self
            .runner
            .driver
            .read_file(f)
            .map_err(|err| self.cx.nid_err(self.nid, RunnerError::ImportIOError(err)))?;
        let module = crate::ast::parser::parse_module(&src);
        let id = self
            .runner
            .ir
            .insert_module(Some(f.as_ref().clone()), src, module)
            .map_err(|err| err.convert().with_trace(self.cx.stacktrace.clone()))?;
        let v = Value::Module(id);
        self.runner.cache.put(
            cache_key,
            CacheEntry {
                execution_time: start.elapsed(),
                expires: None,
                etag: None,
                deps: Vec::new(),
                value: v.clone(),
            },
        );
        Ok(v)
    }

    fn module_file(self) -> ResultValue {
        self.deps.push(Dep::CallingModule);
        self.no_args()?;
        Ok(Value::File(Arc::new(
            self.cx
                .module
                .file()
                .map_err(|err| self.cx.nid_err(self.nid, err))?
                .clone(),
        )))
    }

    fn extract_zip(self) -> ResultValue {
        let f = expect_type!(self, File, single_arg!(self));
        let area = self
            .runner
            .driver
            .extract_zip(f)
            .map_err(|err| self.cx.nid_err(self.nid, err))?;
        Ok(Value::FileArea(Arc::new(area)))
    }

    fn extract_tar_gz(self) -> ResultValue {
        let f = expect_type!(self, File, single_arg!(self));
        let area = self
            .runner
            .driver
            .extract_tar_gz(f)
            .map_err(|err| self.cx.nid_err(self.nid, err))?;
        Ok(Value::FileArea(Arc::new(area)))
    }

    fn extract_tar_xz(self) -> ResultValue {
        let f = expect_type!(self, File, single_arg!(self));
        let area = self
            .runner
            .driver
            .extract_tar_xz(f)
            .map_err(|err| self.cx.nid_err(self.nid, err))?;
        Ok(Value::FileArea(Arc::new(area)))
    }

    fn extract_tar(self) -> ResultValue {
        let f = expect_type!(self, File, single_arg!(self));
        let area = self
            .runner
            .driver
            .extract_tar(f)
            .map_err(|err| self.cx.nid_err(self.nid, err))?;
        Ok(Value::FileArea(Arc::new(area)))
    }

    fn escape_bin(self) -> ResultValue {
        self.check_escape_mode()?;
        self.deps.push(Dep::Escape);
        let name = expect_type!(self, String, single_arg!(self));
        let Some(path) = self.runner.driver.escape_bin(name) else {
            return Ok(Value::Unit);
        };
        Ok(Value::EscapeFile(Arc::new(path)))
    }

    fn unit(self) -> ResultValue {
        *self.cache_hint = false;
        self.no_args()?;
        Ok(Value::Unit)
    }

    fn get_area(self) -> ResultValue {
        let f = expect_type!(self, File, single_arg!(self));
        Ok(Value::FileArea(Arc::new(f.area().clone())))
    }

    fn throw(self) -> ResultValue {
        let (_, err_value) = single_arg!(self);
        Err(self
            .cx
            .module
            .span(self.nid)
            .with_module(self.cx.module.id)
            .with_error(Throwing::Recoverable(err_value.clone()))
            .with_trace(self.cx.stacktrace.clone()))
    }

    fn sha256(self) -> ResultValue {
        let f = expect_type!(self, File, single_arg!(self));
        Ok(Value::String(Arc::new(
            self.runner
                .driver
                .sha256(f)
                .map_err(|err| self.cx.nid_err(self.nid, err))?,
        )))
    }

    fn sha512(self) -> ResultValue {
        let f = expect_type!(self, File, single_arg!(self));
        Ok(Value::String(Arc::new(
            self.runner
                .driver
                .sha512(f)
                .map_err(|err| self.cx.nid_err(self.nid, err))?,
        )))
    }

    fn bytes_to_string(self) -> ResultValue {
        let (bytes_nid, bytes_value) = single_arg!(self);
        let list = expect_type!(self, List, (bytes_nid, bytes_value));
        let bytes = list
            .0
            .iter()
            .map(|b| -> Result<u8> {
                let b = expect_type!(self, Integer, (bytes_nid, b));
                u8::try_from(&b.0).map_err(|err| {
                    self.cx
                        .nid_err(bytes_nid, RunnerError::Makeshift(err.to_string().into()))
                })
            })
            .collect::<Result<Vec<u8>>>()?;
        Ok(Value::String(Arc::new(String::from_utf8(bytes).map_err(
            |err| {
                self.cx
                    .nid_err(bytes_nid, RunnerError::Makeshift(err.to_string().into()))
            },
        )?)))
    }

    fn parse_toml(self) -> ResultValue {
        fn toml_to_rain(v: toml::Value) -> Value {
            match v {
                toml::Value::String(s) => Value::String(Arc::new(s)),
                toml::Value::Integer(n) => Value::Integer(Arc::new(RainInteger(BigInt::from(n)))),
                toml::Value::Float(f) => Value::String(Arc::new(f.to_string())),
                toml::Value::Boolean(b) => Value::Boolean(b),
                toml::Value::Datetime(datetime) => Value::String(Arc::new(datetime.to_string())),
                toml::Value::Array(vec) => Value::List(Arc::new(RainList(
                    vec.into_iter().map(toml_to_rain).collect(),
                ))),
                toml::Value::Table(map) => Value::Record(Arc::new(RainRecord(
                    map.into_iter().map(|(k, v)| (k, toml_to_rain(v))).collect(),
                ))),
            }
        }

        let contents = expect_type!(self, String, single_arg!(self));
        let parsed: toml::Value = toml::de::from_str(contents).map_err(|err| {
            self.cx.nid_err(
                self.nid,
                RunnerError::Makeshift(err.message().to_owned().into()),
            )
        })?;
        Ok(toml_to_rain(parsed))
    }

    fn parse_json(self) -> ResultValue {
        fn json_to_rain(v: serde_json::Value) -> Value {
            match v {
                serde_json::Value::Null => Value::Unit,
                serde_json::Value::String(s) => Value::String(Arc::new(s)),
                serde_json::Value::Number(n) => {
                    if let Some(float) = n.as_f64() {
                        Value::String(Arc::new(float.to_string()))
                    } else {
                        Value::Integer(Arc::new(RainInteger(
                            n.as_i64()
                                .map(BigInt::from)
                                .or_else(|| n.as_u64().map(BigInt::from))
                                .or_else(|| n.as_i128().map(BigInt::from))
                                .or_else(|| n.as_u128().map(BigInt::from))
                                .expect("number not integer"),
                        )))
                    }
                }
                serde_json::Value::Bool(b) => Value::Boolean(b),
                serde_json::Value::Array(vec) => Value::List(Arc::new(RainList(
                    vec.into_iter().map(json_to_rain).collect(),
                ))),
                serde_json::Value::Object(map) => Value::Record(Arc::new(RainRecord(
                    map.into_iter().map(|(k, v)| (k, json_to_rain(v))).collect(),
                ))),
            }
        }

        let contents = expect_type!(self, String, single_arg!(self));
        let parsed: serde_json::Value = serde_json::de::from_str(contents).map_err(|err| {
            self.cx
                .nid_err(self.nid, RunnerError::Makeshift(err.to_string().into()))
        })?;
        Ok(json_to_rain(parsed))
    }

    fn create_area(self) -> ResultValue {
        let (dirs_nid, dirs_value) = single_arg!(self);
        let dirs = expect_type!(self, List, (dirs_nid, dirs_value));
        let dirs: Vec<&FSEntry> = dirs
            .0
            .iter()
            .map(|dir| match dir {
                Value::Dir(d) => Ok(d.inner()),
                Value::File(f) => Ok(f.inner()),
                _ => Err(self.cx.nid_err(
                    dirs_nid,
                    RunnerError::ExpectedType {
                        actual: dir.rain_type_id(),
                        expected: &[RainTypeId::Dir, RainTypeId::File],
                    },
                )),
            })
            .collect::<Result<Vec<&FSEntry>, _>>()?;
        let merged_area = self
            .runner
            .driver
            .create_area(&dirs)
            .map_err(|err| self.cx.nid_err(self.nid, err))?;
        Ok(Value::FileArea(Arc::new(merged_area)))
    }

    fn read_file(self) -> ResultValue {
        let (file_nid, file_value) = single_arg!(self);
        let f = expect_type!(self, File, (file_nid, file_value));
        Ok(Value::String(Arc::new(
            self.runner
                .driver
                .read_file(f)
                .map_err(|err| self.cx.nid_err(self.nid, RunnerError::AreaIOError(err)))?,
        )))
    }

    fn create_file(self) -> ResultValue {
        let (contents, name) = two_args!(self);
        let contents = expect_type!(self, String, contents);
        let name = expect_type!(self, String, name);
        Ok(Value::File(Arc::new(
            self.runner
                .driver
                .create_file(contents, name)
                .map_err(|err| self.cx.nid_err(self.nid, err))?,
        )))
    }

    fn debug(self) -> ResultValue {
        let (_nid, value) = single_arg!(self);
        let p = if let Value::String(s) = &value {
            s.to_string()
        } else {
            format!("{value}")
        };
        self.runner.driver.print(p);
        Ok(value.clone())
    }

    fn local_area(self) -> ResultValue {
        let FileArea::Local(current_area_path) = &self
            .cx
            .module
            .file()
            .map_err(|err| self.cx.nid_err(self.nid, err))?
            .area()
        else {
            return Err(self.cx.nid_err(self.nid, RunnerError::IllegalLocalArea));
        };
        let path = expect_type!(self, String, single_arg!(self));
        let area_path = current_area_path.join(path.as_ref());
        let area_path = AbsolutePathBuf::try_from(area_path.as_path())
            .map_err(|err| self.cx.nid_err(self.nid, RunnerError::AreaIOError(err)))?;
        let entry = FSEntry::new(FileArea::Local(area_path), SealedFilePath::root());
        match self
            .runner
            .driver
            .query_fs(&entry)
            .map_err(|err| self.cx.nid_err(self.nid, RunnerError::AreaIOError(err)))?
        {
            FSEntryQueryResult::Directory => Ok(Value::FileArea(Arc::new(entry.area))),
            result => Err(self
                .cx
                .nid_err(self.nid, RunnerError::FSQuery(entry, result))),
        }
    }

    fn split_string(self) -> ResultValue {
        let (string, sep) = two_args!(self);
        let s = expect_type!(self, String, string);
        let sep = expect_type!(self, String, sep);
        Ok(Value::List(Arc::new(RainList(
            s.split(sep.as_str())
                .map(|s| Value::String(Arc::new(s.to_owned())))
                .collect(),
        ))))
    }

    fn index(self) -> ResultValue {
        let ((indexable_nid, indexable_value), index) = two_args!(self);
        match indexable_value {
            Value::List(list) => {
                let big_int = expect_type!(self, Integer, index);
                let Ok(i) = usize::try_from(&big_int.0) else {
                    return Ok(Value::Unit);
                };
                list.0.get(i).cloned().ok_or_else(|| {
                    self.cx.nid_err(
                        self.nid,
                        RunnerError::IndexOutOfBounds(big_int.as_ref().clone()),
                    )
                })
            }
            Value::Record(record) => {
                let s = expect_type!(self, String, index);
                record.0.get(s.as_str()).cloned().ok_or_else(|| {
                    self.cx.nid_err(
                        self.nid,
                        RunnerError::IndexKeyNotFound(s.as_ref().to_owned()),
                    )
                })
            }
            _ => Err(self.cx.nid_err(
                indexable_nid,
                RunnerError::ExpectedType {
                    actual: indexable_value.rain_type_id(),
                    expected: &[RainTypeId::List, RainTypeId::Record],
                },
            )),
        }
    }

    fn host_info(self) -> ResultValue {
        self.no_args()?;
        let mut record = IndexMap::new();
        let host_triple = self.runner.driver.host_triple();
        record.insert(
            "triple".into(),
            Value::String(Arc::new(String::from(host_triple))),
        );
        record.insert(
            "rain_version".into(),
            Value::String(Arc::new(String::from(env!("CARGO_PKG_VERSION")))),
        );
        Ok(Value::Record(Arc::new(RainRecord(record))))
    }

    fn string_contains(self) -> ResultValue {
        let (haystack, needle) = two_args!(self);
        let haystack = expect_type!(self, String, haystack);
        let needle = expect_type!(self, String, needle);
        Ok(Value::Boolean(haystack.contains(&**needle)))
    }

    #[expect(clippy::too_many_lines)]
    fn export_to_local(self) -> ResultValue {
        match &self.arg_values[..] {
            [(src_nid, src_value), (dst_nid, dst_value)] => {
                let dst = self.expect_dir_or_area(*dst_nid, dst_value)?;
                match dst.area() {
                    FileArea::Local(_) => (),
                    FileArea::Generated(_) => {
                        return Err(self.cx.nid_err(
                            *dst_nid,
                            RunnerError::Makeshift("destination must be in a local area".into()),
                        ));
                    }
                }
                match src_value {
                    Value::File(src) => {
                        let filename = src.path().last().ok_or_else(|| {
                            self.cx.nid_err(
                                src_nid,
                                RunnerError::Makeshift("src path does not have filename".into()),
                            )
                        })?;
                        let dst_path = dst.path().join(filename).map_err(|err| {
                            self.cx.nid_err(self.nid, RunnerError::PathError(err))
                        })?;
                        let dst = FSEntry::new(dst.area().clone(), dst_path);

                        self.runner
                            .driver
                            .export_file(src, &dst)
                            .map_err(|err| self.cx.nid_err(self.nid, err))?;
                        Ok(Value::Unit)
                    }
                    Value::Dir(src) => {
                        let filename = src.path().last().ok_or_else(|| {
                            self.cx.nid_err(
                                src_nid,
                                RunnerError::Makeshift("src path does not have last part".into()),
                            )
                        })?;
                        let dst_path = dst.path().join(filename).map_err(|err| {
                            self.cx.nid_err(self.nid, RunnerError::PathError(err))
                        })?;
                        let dst = FSEntry::new(dst.area().clone(), dst_path);

                        self.runner
                            .driver
                            .export_dir(src, &dst)
                            .map_err(|err| self.cx.nid_err(self.nid, err))?;
                        Ok(Value::Unit)
                    }
                    _ => Err(self.cx.nid_err(
                        src_nid,
                        RunnerError::ExpectedType {
                            actual: src_value.rain_type_id(),
                            expected: &[RainTypeId::File, RainTypeId::Dir],
                        },
                    )),
                }
            }
            [
                (src_nid, src_value),
                (dst_nid, dst_value),
                (filename_nid, filename_value),
            ] => {
                let dst = self.expect_dir_or_area(*dst_nid, dst_value)?;
                let filename = expect_type!(self, String, (filename_nid, filename_value));
                match dst.area() {
                    FileArea::Local(_) => (),
                    FileArea::Generated(_) => {
                        return Err(self.cx.nid_err(
                            *dst_nid,
                            RunnerError::Makeshift("destination must be in a local area".into()),
                        ));
                    }
                }

                let dst_path = dst
                    .path()
                    .join(filename)
                    .map_err(|err| self.cx.nid_err(self.nid, RunnerError::PathError(err)))?;
                let dst = FSEntry::new(dst.area().clone(), dst_path);

                match src_value {
                    Value::File(src) => {
                        self.runner
                            .driver
                            .export_file(src, &dst)
                            .map_err(|err| self.cx.nid_err(self.nid, err))?;
                        Ok(Value::Unit)
                    }
                    Value::Dir(src) => {
                        self.runner
                            .driver
                            .export_dir(src, &dst)
                            .map_err(|err| self.cx.nid_err(self.nid, err))?;
                        Ok(Value::Unit)
                    }
                    Value::FileArea(area) => {
                        let src = Dir::root(area.as_ref().clone());
                        self.runner
                            .driver
                            .export_dir(&src, &dst)
                            .map_err(|err| self.cx.nid_err(self.nid, err))?;
                        Ok(Value::Unit)
                    }
                    _ => Err(self.cx.nid_err(
                        src_nid,
                        RunnerError::ExpectedType {
                            actual: src_value.rain_type_id(),
                            expected: &[RainTypeId::File, RainTypeId::Dir, RainTypeId::FileArea],
                        },
                    )),
                }
            }
            _ => self.incorrect_args(2..=3),
        }
    }

    #[expect(clippy::too_many_lines)]
    fn check_export_to_local(self) -> ResultValue {
        match &self.arg_values[..] {
            [(src_nid, src_value), (dst_nid, dst_value)] => {
                let src = expect_type!(self, File, (src_nid, src_value));
                let dst = expect_type!(self, Dir, (dst_nid, dst_value));
                match dst.area() {
                    FileArea::Local(_) => (),
                    FileArea::Generated(_) => {
                        return Err(self.cx.nid_err(
                            *dst_nid,
                            RunnerError::Makeshift("destination must be in a local area".into()),
                        ));
                    }
                }
                let filename = src.path().last().ok_or_else(|| {
                    self.cx.nid_err(
                        self.nid,
                        RunnerError::Makeshift("src path does not have filename".into()),
                    )
                })?;
                let dst_path = dst
                    .path()
                    .join(filename)
                    .map_err(|err| self.cx.nid_err(self.nid, RunnerError::PathError(err)))?;
                let entry = FSEntry::new(dst.area().clone(), dst_path);
                match self
                    .runner
                    .driver
                    .query_fs(&entry)
                    .map_err(|err| self.cx.nid_err(self.nid, RunnerError::AreaIOError(err)))?
                {
                    FSEntryQueryResult::File => {}
                    _ => {
                        return Err(self.cx.nid_err(
                            self.nid,
                            RunnerError::Makeshift("exported file does not exist".into()),
                        ));
                    }
                }
                // Safety: We just checked this
                let dst = unsafe { File::new(entry) };
                let src_contents = self
                    .runner
                    .driver
                    .read_file(src)
                    .map_err(|err| self.cx.nid_err(self.nid, RunnerError::AreaIOError(err)))?;
                let dst_contents = self
                    .runner
                    .driver
                    .read_file(&dst)
                    .map_err(|err| self.cx.nid_err(self.nid, RunnerError::AreaIOError(err)))?;
                if src_contents != dst_contents {
                    return Err(self.cx.nid_err(
                        self.nid,
                        RunnerError::Makeshift("exported file does not match".into()),
                    ));
                }

                Ok(Value::Unit)
            }
            [
                (src_nid, src_value),
                (dst_nid, dst_value),
                (filename_nid, filename_value),
            ] => {
                let src = expect_type!(self, File, (src_nid, src_value));
                let dst = expect_type!(self, Dir, (dst_nid, dst_value));
                let filename = expect_type!(self, String, (filename_nid, filename_value));
                match dst.area() {
                    FileArea::Local(_) => (),
                    FileArea::Generated(_) => {
                        return Err(self.cx.nid_err(
                            *dst_nid,
                            RunnerError::Makeshift("destination must be in a local area".into()),
                        ));
                    }
                }
                let dst_path = dst
                    .path()
                    .join(filename)
                    .map_err(|err| self.cx.nid_err(self.nid, RunnerError::PathError(err)))?;
                let entry = FSEntry::new(dst.area().clone(), dst_path);
                match self
                    .runner
                    .driver
                    .query_fs(&entry)
                    .map_err(|err| self.cx.nid_err(self.nid, RunnerError::AreaIOError(err)))?
                {
                    FSEntryQueryResult::File => {}
                    _ => {
                        return Err(self.cx.nid_err(
                            self.nid,
                            RunnerError::Makeshift("exported file does not exist".into()),
                        ));
                    }
                }
                // Safety: We just checked this
                let dst = unsafe { File::new(entry) };
                let src_contents = self
                    .runner
                    .driver
                    .read_file(src)
                    .map_err(|err| self.cx.nid_err(self.nid, RunnerError::AreaIOError(err)))?;
                let dst_contents = self
                    .runner
                    .driver
                    .read_file(&dst)
                    .map_err(|err| self.cx.nid_err(self.nid, RunnerError::AreaIOError(err)))?;
                if src_contents != dst_contents {
                    return Err(self.cx.nid_err(
                        self.nid,
                        RunnerError::Makeshift("exported file does not match".into()),
                    ));
                }

                Ok(Value::Unit)
            }
            _ => self.incorrect_args(2..=3),
        }
    }

    fn file_metadata(self) -> ResultValue {
        let f = expect_type!(self, File, single_arg!(self));
        let metadata = self
            .runner
            .driver
            .file_metadata(f)
            .map_err(|err| self.cx.nid_err(self.nid, err))?;
        let mut record = IndexMap::new();
        record.insert(
            "size".to_owned(),
            Value::Integer(Arc::new(RainInteger(metadata.size.into()))),
        );
        Ok(Value::Record(Arc::new(RainRecord(record))))
    }

    fn glob(self) -> ResultValue {
        match &self.arg_values[..] {
            [(dir_nid, dir_value)] => {
                let d = match dir_value {
                    Value::Dir(d) => d.as_ref(),
                    Value::FileArea(a) => &Dir::root((**a).clone()),
                    _ => {
                        return Err(self.cx.nid_err(
                            *dir_nid,
                            RunnerError::ExpectedType {
                                actual: dir_value.rain_type_id(),
                                expected: &[RainTypeId::Dir, RainTypeId::FileArea],
                            },
                        ));
                    }
                };
                let files = self
                    .runner
                    .driver
                    .glob(d, "**/*")
                    .map_err(|err| self.cx.nid_err(self.nid, err))?;
                let files: Vec<Value> = files
                    .into_iter()
                    .map(|f| Value::File(Arc::new(f)))
                    .collect();
                Ok(Value::List(Arc::new(RainList(files))))
            }
            [_, _] => Err(self.cx.nid_err(
                self.nid,
                RunnerError::Makeshift(
                    "implement globbing, for now you can only use globbing with a single arg"
                        .into(),
                ),
            )),
            _ => self.incorrect_args(1..=2),
        }
    }

    fn foreach(self) -> ResultValue {
        let (list, (func_nid, func_value)) = two_args!(self);
        let list = expect_type!(self, List, list);
        let mut out = Vec::with_capacity(list.0.len());
        for item in list.0.clone() {
            let arg_values: Vec<(NodeId, Value)> = vec![(func_nid, item)];
            out.push(self.runner.call_function(
                self.cx,
                self.nid,
                func_value,
                self.call_span,
                arg_values,
            )?);
        }
        Ok(Value::List(Arc::new(RainList(out))))
    }

    fn stringify(self) -> ResultValue {
        let (nid, value) = single_arg!(self);
        match value {
            Value::File(f) => Ok(Value::String(Arc::new(
                self.runner
                    .driver
                    .resolve_fs_entry(f.inner())
                    .display()
                    .to_string(),
            ))),
            Value::Dir(d) => Ok(Value::String(Arc::new(
                self.runner
                    .driver
                    .resolve_fs_entry(d.inner())
                    .display()
                    .to_string(),
            ))),
            Value::EscapeFile(f) => Ok(Value::String(Arc::new(format!("{}", f.0.display())))),
            _ => Err(self.cx.nid_err(
                nid,
                RunnerError::ExpectedType {
                    actual: value.rain_type_id(),
                    expected: &[RainTypeId::File, RainTypeId::Dir],
                },
            )),
        }
    }

    fn prelude(self) -> ResultValue {
        self.no_args()?;
        let cache_key = CacheKey::Prelude;
        if let Some(v) = self.runner.cache.get_value(&cache_key) {
            return Ok(v);
        }
        let start = Instant::now();
        let Some(src) = self.runner.driver.prelude_src() else {
            return Err(self
                .cx
                .nid_err(self.nid, RunnerError::Makeshift("no prelude".into())));
        };
        let module = crate::ast::parser::parse_module(src.as_ref());
        let id = self
            .runner
            .ir
            .insert_module(None, src, module)
            .map_err(|err| err.convert().with_trace(self.cx.stacktrace.clone()))?;
        let v = Value::Module(id);
        self.runner.cache.put(
            cache_key,
            CacheEntry {
                execution_time: start.elapsed(),
                expires: None,
                etag: None,
                deps: Vec::new(),
                value: v.clone(),
            },
        );
        Ok(v)
    }

    fn create_tar(self) -> ResultValue {
        let ((dir_nid, dir_value), name) = two_args!(self);
        let dir = self.expect_dir_or_area(dir_nid, dir_value)?;
        let name = expect_type!(self, String, name);
        Ok(Value::File(Arc::new(
            self.runner
                .driver
                .create_tar(&dir, name)
                .map_err(|err| self.cx.nid_err(self.nid, err))?,
        )))
    }

    fn create_tar_gz(self) -> ResultValue {
        let ((dir_nid, dir_value), name) = two_args!(self);
        let dir = self.expect_dir_or_area(dir_nid, dir_value)?;
        let name = expect_type!(self, String, name);
        Ok(Value::File(Arc::new(
            self.runner
                .driver
                .create_tar_gz(&dir, name)
                .map_err(|err| self.cx.nid_err(self.nid, err))?,
        )))
    }

    fn rust_eq(self) -> ResultValue {
        *self.cache_hint = false;
        let ((_, a), (_, b)) = two_args!(self);
        Ok(Value::Boolean(a == b))
    }

    fn get_secret(self) -> ResultValue {
        let name = expect_type!(self, String, single_arg!(self));
        self.deps.push(Dep::Secret);
        let secret = self
            .runner
            .driver
            .get_secret(name)
            .map_err(|err| self.cx.nid_err(self.nid, err))?;
        Ok(Value::String(Arc::new(secret)))
    }

    fn set_cache_never(self) -> ResultValue {
        self.no_args()?;
        self.deps.push(Dep::Uncacheable);
        Ok(Value::Unit)
    }

    fn clear_calling_cache_deps(self) -> ResultValue {
        self.no_args()?;
        self.cx.deps.clear();
        Ok(Value::Unit)
    }

    fn merge_records(self) -> ResultValue {
        let (record1, record2) = two_args!(self);
        let record1 = expect_type!(self, Record, record1);
        let record2 = expect_type!(self, Record, record2);
        let mut out_record = record1.as_ref().clone();
        for (k, v) in &record2.as_ref().0 {
            out_record.0.insert(k.clone(), v.clone());
        }
        Ok(Value::Record(Arc::new(out_record)))
    }

    fn parse_target_triple(self) -> ResultValue {
        let triple = expect_type!(self, String, single_arg!(self));
        let triple = match target_lexicon::Triple::from_str(triple) {
            Ok(triple) => triple,
            Err(err) => {
                return Err(self.cx.nid_err(
                    self.nid,
                    RunnerError::Makeshift(std::borrow::Cow::Owned(err.to_string())),
                ));
            }
        };
        let mut out = IndexMap::new();
        out.insert(
            "arch".into(),
            Value::String(Arc::new(triple.architecture.to_string())),
        );
        out.insert(
            "vendor".into(),
            Value::String(Arc::new(triple.vendor.to_string())),
        );
        out.insert(
            "os".into(),
            Value::String(Arc::new(triple.operating_system.to_string())),
        );
        out.insert(
            "env".into(),
            Value::String(Arc::new(triple.environment.to_string())),
        );
        out.insert(
            "bin".into(),
            Value::String(Arc::new(triple.binary_format.to_string())),
        );
        Ok(Value::Record(Arc::new(RainRecord(out))))
    }

    fn git_contents(self) -> ResultValue {
        let (url, commit) = two_args!(self);
        let url = expect_type!(self, String, url);
        let commit = expect_type!(self, String, commit);
        let area = self
            .runner
            .driver
            .git_contents(url, commit)
            .map_err(|err| self.cx.nid_err(self.nid, err))?;
        Ok(Value::FileArea(Arc::new(area)))
    }

    fn git_lfs_smudge(self) -> ResultValue {
        let area = expect_type!(self, FileArea, single_arg!(self));
        let new_area = self
            .runner
            .driver
            .git_lfs_smudge(area)
            .map_err(|err| self.cx.nid_err(self.nid, err))?;
        Ok(Value::FileArea(Arc::new(new_area)))
    }

    fn env_var(self) -> ResultValue {
        let var_name = expect_type!(self, String, single_arg!(self));
        if let Some(value) = self
            .runner
            .driver
            .env_var(var_name)
            .map_err(|err| self.cx.nid_err(self.nid, err))?
        {
            Ok(Value::String(Arc::new(value)))
        } else {
            Ok(Value::Unit)
        }
    }

    fn copy_file(self) -> ResultValue {
        let (file, name) = two_args!(self);
        let file = expect_type!(self, File, file);
        let name = expect_type!(self, String, name);
        let new_file = self
            .runner
            .driver
            .copy_file(file, name)
            .map_err(|err| self.cx.nid_err(self.nid, err))?;
        Ok(Value::File(Arc::new(new_file)))
    }

    fn list_length(self) -> ResultValue {
        let list = expect_type!(self, List, single_arg!(self));
        Ok(Value::Integer(Arc::new(RainInteger(BigInt::from(
            list.0.len(),
        )))))
    }

    fn escape_hard(self) -> ResultValue {
        let file_path = expect_type!(self, String, single_arg!(self));
        Ok(Value::EscapeFile(Arc::new(
            AbsolutePathBuf::try_from(Path::new(file_path.as_str())).map_err(|err| {
                self.cx.nid_err(
                    self.nid,
                    RunnerError::MakeshiftIO("absolute path".into(), err),
                )
            })?,
        )))
    }

    fn flatten(self) -> ResultValue {
        let list = expect_type!(self, List, single_arg!(self));
        let mut flattened = Vec::new();
        for value in &list.0 {
            let inner_list = expect_type!(self, List, (self.nid, value));
            for v in &inner_list.0 {
                flattened.push(v.clone());
            }
        }
        Ok(Value::List(Arc::new(RainList(flattened))))
    }

    fn get_type(self) -> ResultValue {
        let (_arg_nid, arg_value) = single_arg!(self);
        Ok(Value::Type(arg_value.rain_type_id()))
    }
}
