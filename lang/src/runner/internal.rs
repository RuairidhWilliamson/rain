#![allow(clippy::unnecessary_wraps)]

mod archives;
mod fs;
mod http;
mod macros;
mod run;

use std::{
    borrow::Cow,
    hash::Hash,
    ops::RangeInclusive,
    path::{Path, PathBuf},
    str::FromStr as _,
    sync::{Arc, atomic::Ordering},
    time::Instant,
};

use alias::Alias as _;
use indexmap::IndexMap;
use num_bigint::BigInt;
use tracing::{debug, trace};

use crate::{
    afs::{Dir, File, absolute::AbsolutePathBuf, area::FSArea},
    ast::{Module, NodeId},
    driver::DriverTrait,
    local_span::LocalSpan,
    runner::{
        Result,
        cache::{CacheEntry, CacheKey, CacheTrait},
        cx::Cx,
        dep::Dep,
        dep_list::DepList,
        error::{RunnerError, Throwing},
        internal::macros::{expect_type, unpack_args},
        value::{RainInteger, RainList, RainRecord, RainTypeId, Value},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum InternalFunction {
    BytesToString,
    CheckExportToLocal,
    ClearCallingCacheDeps,
    CompressGzip,
    CompressZstd,
    CopyFile,
    CreateArea,
    CreateFile,
    CreateTar,
    CreateWriteArea,
    Debug,
    Download,
    Embed,
    EnvVar,
    EscapeBin,
    EscapeHard,
    EscapeRun,
    ExportToLocal,
    ExtractGzip,
    ExtractTar,
    ExtractXz,
    ExtractZip,
    ExtractZstd,
    FileMetadata,
    Fold,
    GetArea,
    GetDir,
    GetFile,
    GetSecret,
    GetType,
    GitContents,
    GitLfsSmudge,
    Glob,
    HostInfo,
    Import,
    Index,
    LocalArea,
    MergeRecords,
    ModuleFile,
    ParseJSON,
    ParseTargetTriple,
    ParseToml,
    Print,
    ReadFile,
    RecordKeys,
    Run,
    RustEq,
    SetCacheNever,
    Sha256,
    Sha512,
    SplitString,
    StringContains,
    StringReplaceAll,
    RegexReplaceF,
    Stringify,
    Throw,
    Unit,
    FileName,
    CopyDir,
    Config,
    ConcreteTypes,
    IncCounter,
    Try,
    CreateUnique,
    Offline,
    GitDescribe,
    HttpPost,
}

impl std::fmt::Display for InternalFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

impl InternalFunction {
    pub fn evaluate_internal_function_name(name: &str) -> Option<Self> {
        match name {
            "_bytes_to_string" => Some(Self::BytesToString),
            "_check_export_to_local" => Some(Self::CheckExportToLocal),
            "_clear_calling_cache_deps" => Some(Self::ClearCallingCacheDeps),
            "_compress_gzip" => Some(Self::CompressGzip),
            "_compress_zstd" => Some(Self::CompressZstd),
            "_copy_file" => Some(Self::CopyFile),
            "_create_area" => Some(Self::CreateArea),
            "_create_file" => Some(Self::CreateFile),
            "_create_tar" => Some(Self::CreateTar),
            "_create_write_area" => Some(Self::CreateWriteArea),
            "_debug" => Some(Self::Debug),
            "_download" => Some(Self::Download),
            "_embed" => Some(Self::Embed),
            "_env_var" => Some(Self::EnvVar),
            "_escape_bin" => Some(Self::EscapeBin),
            "_escape_hard" => Some(Self::EscapeHard),
            "_escape_run" => Some(Self::EscapeRun),
            "_export_to_local" => Some(Self::ExportToLocal),
            "_extract_gzip" => Some(Self::ExtractGzip),
            "_extract_tar" => Some(Self::ExtractTar),
            "_extract_xz" => Some(Self::ExtractXz),
            "_extract_zip" => Some(Self::ExtractZip),
            "_extract_zstd" => Some(Self::ExtractZstd),
            "_file_metadata" => Some(Self::FileMetadata),
            "_fold" => Some(Self::Fold),
            "_get_area" => Some(Self::GetArea),
            "_get_dir" => Some(Self::GetDir),
            "_get_file" => Some(Self::GetFile),
            "_get_secret" => Some(Self::GetSecret),
            "_get_type" => Some(Self::GetType),
            "_git_contents" => Some(Self::GitContents),
            "_git_lfs_smudge" => Some(Self::GitLfsSmudge),
            "_glob" => Some(Self::Glob),
            "_host_info" => Some(Self::HostInfo),
            "_import" => Some(Self::Import),
            "_index" => Some(Self::Index),
            "_local_area" => Some(Self::LocalArea),
            "_merge_records" => Some(Self::MergeRecords),
            "_module_file" => Some(Self::ModuleFile),
            "_parse_json" => Some(Self::ParseJSON),
            "_parse_target_triple" => Some(Self::ParseTargetTriple),
            "_parse_toml" => Some(Self::ParseToml),
            "_print" => Some(Self::Print),
            "_read_file" => Some(Self::ReadFile),
            "_record_keys" => Some(Self::RecordKeys),
            "_run" => Some(Self::Run),
            "_rust_eq" => Some(Self::RustEq),
            "_set_cache_never" => Some(Self::SetCacheNever),
            "_sha256" => Some(Self::Sha256),
            "_sha512" => Some(Self::Sha512),
            "_split_string" => Some(Self::SplitString),
            "_string_contains" => Some(Self::StringContains),
            "_string_replace_all" => Some(Self::StringReplaceAll),
            "_regex_replace_f" => Some(Self::RegexReplaceF),
            "_stringify" => Some(Self::Stringify),
            "_throw" => Some(Self::Throw),
            "_unit" => Some(Self::Unit),
            "_file_name" => Some(Self::FileName),
            "_copy_dir" => Some(Self::CopyDir),
            "_config" => Some(Self::Config),
            "_concrete_types" => Some(Self::ConcreteTypes),
            "_inc_counter" => Some(Self::IncCounter),
            "_try" => Some(Self::Try),
            "_create_unique" => Some(Self::CreateUnique),
            "_offline" => Some(Self::Offline),
            "_git_describe" => Some(Self::GitDescribe),
            "_http_post" => Some(Self::HttpPost),
            _ => None,
        }
    }
}

pub struct InternalCx<'a, 'b, 'c, Driver, Cache> {
    pub func: InternalFunction,
    pub runner: &'a mut super::Runner<'c, Driver, Cache>,
    /// The calling function's cx
    /// Deps should not be added to this but to [`deps`]
    pub caller_cx: &'a mut Cx<'b>,
    pub nid: NodeId,
    pub call_span: LocalSpan,
    pub arg_values: Vec<(NodeId, Value)>,
    pub deps: &'a mut DepList,
    /// Set to false to hint to the caller that this is probably less efficient to store in cache
    pub cache_hint: &'a mut bool,
}

impl<Driver: DriverTrait, Cache: CacheTrait> InternalCx<'_, '_, '_, Driver, Cache> {
    pub fn call_internal_function(self) -> Result<Value> {
        trace!("call {:?} with {:?}", self.func, self.arg_values);
        match self.func {
            InternalFunction::Print => self.print(),
            InternalFunction::Debug => self.debug(),
            InternalFunction::GetFile => self.get_file(),
            InternalFunction::GetDir => self.get_dir(),
            InternalFunction::Import => self.import(),
            InternalFunction::ModuleFile => self.module_file(),
            InternalFunction::ExtractZip => self.extract_zip(),
            InternalFunction::ExtractGzip => self.extract_gzip(),
            InternalFunction::ExtractXz => self.extract_xz(),
            InternalFunction::ExtractTar => self.extract_tar(),
            InternalFunction::Run => self.run(),
            InternalFunction::EscapeBin => self.escape_bin(),
            InternalFunction::Unit => self.unit(),
            InternalFunction::GetArea => self.get_area(),
            InternalFunction::Download => self.download(),
            InternalFunction::Throw => self.throw(),
            InternalFunction::Sha256 => self.sha256(),
            InternalFunction::Sha512 => self.sha512(),
            InternalFunction::BytesToString => self.bytes_to_string(),
            InternalFunction::ParseToml => self.parse_toml(),
            InternalFunction::CreateArea => self.create_area(),
            InternalFunction::ReadFile => self.read_file(),
            InternalFunction::CreateFile => self.create_file(),
            InternalFunction::LocalArea => self.local_area(),
            InternalFunction::SplitString => self.split_string(),
            InternalFunction::Index => self.index(),
            InternalFunction::HostInfo => self.host_info(),
            InternalFunction::StringContains => self.string_contains(),
            InternalFunction::StringReplaceAll => self.string_replace_all(),
            InternalFunction::ExportToLocal => self.export_to_local(),
            InternalFunction::CheckExportToLocal => self.check_export_to_local(),
            InternalFunction::FileMetadata => self.file_metadata(),
            InternalFunction::Glob => self.glob(),
            InternalFunction::Stringify => self.stringify(),
            InternalFunction::EscapeRun => self.escape_run(),
            InternalFunction::Embed => self.embed(),
            InternalFunction::CreateTar => self.create_tar(),
            InternalFunction::RustEq => self.rust_eq(),
            InternalFunction::GetSecret => self.get_secret(),
            InternalFunction::SetCacheNever => self.set_cache_never(),
            InternalFunction::ClearCallingCacheDeps => self.clear_calling_cache_deps(),
            InternalFunction::MergeRecords => self.merge_records(),
            InternalFunction::ParseTargetTriple => self.parse_target_triple(),
            InternalFunction::GitContents => self.git_contents(),
            InternalFunction::GitLfsSmudge => self.git_lfs_smudge(),
            InternalFunction::EnvVar => self.env_var(),
            InternalFunction::CopyFile => self.copy_file(),
            InternalFunction::EscapeHard => self.escape_hard(),
            InternalFunction::CompressGzip => self.compress_gzip(),
            InternalFunction::ParseJSON => self.parse_json(),
            InternalFunction::GetType => self.get_type(),
            InternalFunction::CreateWriteArea => self.create_write_area(),
            InternalFunction::Fold => self.fold(),
            InternalFunction::RecordKeys => self.record_keys(),
            InternalFunction::CompressZstd => self.compress_zstd(),
            InternalFunction::ExtractZstd => self.extract_zstd(),
            InternalFunction::FileName => self.file_name(),
            InternalFunction::CopyDir => self.copy_dir(),
            InternalFunction::Config => self.config(),
            InternalFunction::ConcreteTypes => self.concrete_types(),
            InternalFunction::IncCounter => self.inc_counter(),
            InternalFunction::Try => self.try_function(),
            InternalFunction::CreateUnique => self.create_unique(),
            InternalFunction::Offline => self.offline(),
            InternalFunction::RegexReplaceF => self.regex_replace_f(),
            InternalFunction::GitDescribe => self.git_describe(),
            InternalFunction::HttpPost => self.http_post(),
        }
    }

    fn add_deps_from_args(&mut self) {
        for (_, a) in &self.arg_values {
            self.deps.add_based_on_value(a);
        }
    }

    fn no_args(&self) -> Result<()> {
        if self.arg_values.is_empty() {
            Ok(())
        } else {
            Err(self.caller_cx.err(
                self.call_span,
                RunnerError::IncorrectArgs {
                    required: 0..=0,
                    actual: self.arg_values.len(),
                },
            ))
        }
    }

    fn incorrect_args<T>(&self, required: RangeInclusive<usize>) -> Result<T> {
        Err(self.caller_cx.err(
            self.call_span,
            RunnerError::IncorrectArgs {
                required,
                actual: self.arg_values.len(),
            },
        ))
    }

    fn expect_fs_area(&self, (arg_nid, arg_value): (NodeId, &Value)) -> Result<FSArea> {
        match arg_value {
            Value::GeneratedFSArea(area) => Ok(FSArea::Generated(area.as_ref().clone())),
            Value::LocalFSArea(area) => Ok(FSArea::Local(area.as_ref().clone())),
            _ => Err(self.caller_cx.nid_err(
                arg_nid,
                RunnerError::ExpectedType {
                    actual: arg_value.rain_type_id(),
                    expected: Cow::Borrowed(&[RainTypeId::GeneratedFile, RainTypeId::LocalFile]),
                },
            )),
        }
    }

    fn expect_file(&self, (arg_nid, arg_value): (NodeId, &Value)) -> Result<File> {
        match arg_value {
            Value::GeneratedFile(file) => Ok(File::Generated(file.as_ref().clone())),
            Value::LocalFile(file) => Ok(File::Local(file.as_ref().clone())),
            _ => Err(self.caller_cx.nid_err(
                arg_nid,
                RunnerError::ExpectedType {
                    actual: arg_value.rain_type_id(),
                    expected: Cow::Borrowed(&[RainTypeId::GeneratedFile, RainTypeId::LocalFile]),
                },
            )),
        }
    }

    fn expect_file_path(&self, (arg_nid, arg_value): (NodeId, &Value)) -> Result<PathBuf> {
        match arg_value {
            Value::GeneratedFile(file) => {
                Ok(self.runner.driver.resolve_fs_entry(file.fsinner().into()))
            }
            Value::LocalFile(file) => {
                Ok(self.runner.driver.resolve_fs_entry(file.fsinner().into()))
            }
            Value::EscapeFile(file) => Ok(file.to_path_buf()),
            _ => Err(self.caller_cx.nid_err(
                arg_nid,
                RunnerError::ExpectedType {
                    actual: arg_value.rain_type_id(),
                    expected: Cow::Borrowed(&[RainTypeId::GeneratedFile, RainTypeId::LocalFile]),
                },
            )),
        }
    }

    fn expect_dir_or_area(&self, (arg_nid, arg_value): (NodeId, &Value)) -> Result<Dir> {
        match arg_value {
            Value::LocalFSArea(file_area) => Ok(Dir::root(file_area.as_ref().into())),
            Value::GeneratedFSArea(file_area) => Ok(Dir::root(file_area.as_ref().into())),
            Value::GeneratedDir(dir) => Ok(Dir::Generated(dir.as_ref().clone())),
            Value::LocalDir(dir) => Ok(Dir::Local(dir.as_ref().clone())),
            _ => Err(self.caller_cx.nid_err(
                arg_nid,
                RunnerError::ExpectedType {
                    actual: arg_value.rain_type_id(),
                    expected: Cow::Borrowed(&[
                        RainTypeId::GeneratedDir,
                        RainTypeId::GeneratedFSArea,
                        RainTypeId::LocalFSArea,
                    ]),
                },
            )),
        }
    }

    fn check_escape_mode(&self) -> Result<()> {
        if self.runner.seal {
            Err(self
                .caller_cx
                .nid_err(self.nid, RunnerError::CantEscapeSeal))
        } else {
            Ok(())
        }
    }

    fn print(self) -> Result<Value> {
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

    fn import(mut self) -> Result<Value> {
        self.add_deps_from_args();
        *self.cache_hint = false;
        let f = self.expect_file(unpack_args!(self, 1))?;
        let cache_key = CacheKey::Import { file: f.clone() };
        if let Some(v) = self.runner.cache.get_value(
            &cache_key,
            self.runner.driver,
            &mut self.runner.local_file_hash_cache,
        ) {
            return Ok(v);
        }
        let start = Instant::now();
        let src = self.runner.driver.read_file(&f).map_err(|err| {
            self.caller_cx
                .nid_err(self.nid, RunnerError::ImportIOError(err))
        })?;
        let module = Module::parse(&src);
        let id = self
            .runner
            .ir
            .insert_module(Some(f), src, module)
            .map_err(|err| err.convert().with_trace(self.caller_cx.stacktrace.clone()))?;
        let module = self.runner.ir.get_module(id);
        let mut check_result = crate::runner::checker::CheckModuleResult::check_module(
            module,
            self.runner.check_unused,
        );
        check_result.errors.truncate(1);
        if let Some(err) = check_result.errors.pop() {
            return Err(err
                .upgrade(id)
                .convert::<RunnerError>()
                .convert::<Throwing>()
                .with_trace(self.caller_cx.stacktrace.clone()));
        }
        let v = Value::Module(id);
        self.runner.cache.put(
            cache_key,
            CacheEntry {
                execution_time: start.elapsed(),
                expires: None,
                etag: None,
                deps: DepList::new(),
                value: v.clone(),
            },
        );
        Ok(v)
    }

    fn module_file(self) -> Result<Value> {
        self.deps.push(Dep::CallingModule);
        self.no_args()?;
        Ok(self
            .caller_cx
            .module
            .file()
            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?
            .clone()
            .to_value())
    }

    fn escape_bin(self) -> Result<Value> {
        self.check_escape_mode()?;
        self.deps.push(Dep::Escape);
        let name = expect_type!(self, String, unpack_args!(self, 1));
        let Some(path) = self.runner.driver.escape_bin(name) else {
            return Ok(Value::Unit);
        };
        Ok(Value::EscapeFile(Arc::new(path)))
    }

    fn unit(self) -> Result<Value> {
        *self.cache_hint = false;
        self.no_args()?;
        Ok(Value::Unit)
    }

    fn throw(self) -> Result<Value> {
        *self.cache_hint = false;
        let (_, err_value) = unpack_args!(self, 1);
        Err(self
            .caller_cx
            .module
            .span(self.nid)
            .with_module(self.caller_cx.module.id)
            .with_error(Throwing::Recoverable(err_value.clone()))
            .with_trace(self.caller_cx.stacktrace.clone()))
    }

    fn bytes_to_string(self) -> Result<Value> {
        let (bytes_nid, bytes_value) = unpack_args!(self, 1);
        let list = expect_type!(self, List, (bytes_nid, bytes_value));
        let bytes = list
            .0
            .iter()
            .map(|b| -> Result<u8> {
                let b = expect_type!(self, Integer, (bytes_nid, b));
                u8::try_from(&b.0).map_err(|err| {
                    self.caller_cx
                        .nid_err(bytes_nid, RunnerError::Makeshift(err.to_string().into()))
                })
            })
            .collect::<Result<Vec<u8>>>()?;
        Ok(Value::String(Arc::new(String::from_utf8(bytes).map_err(
            |err| {
                self.caller_cx
                    .nid_err(bytes_nid, RunnerError::Makeshift(err.to_string().into()))
            },
        )?)))
    }

    fn parse_toml(self) -> Result<Value> {
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

        let contents = expect_type!(self, String, unpack_args!(self, 1));
        let parsed: toml::Value = toml::de::from_str(contents).map_err(|err| {
            self.caller_cx.nid_err(
                self.nid,
                RunnerError::Makeshift(err.message().to_owned().into()),
            )
        })?;
        Ok(toml_to_rain(parsed))
    }

    fn parse_json(self) -> Result<Value> {
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

        let contents = expect_type!(self, String, unpack_args!(self, 1));
        let parsed: serde_json::Value = serde_json::de::from_str(contents).map_err(|err| {
            self.caller_cx
                .nid_err(self.nid, RunnerError::Makeshift(err.to_string().into()))
        })?;
        Ok(json_to_rain(parsed))
    }

    fn debug(self) -> Result<Value> {
        let (_nid, value) = unpack_args!(self, 1);
        let p = if let Value::String(s) = &value {
            s.to_string()
        } else {
            format!("{value}")
        };
        self.runner.driver.print(p);
        Ok(value.clone())
    }

    fn split_string(self) -> Result<Value> {
        let (string, sep) = unpack_args!(self, 2);
        let s = expect_type!(self, String, string);
        let sep = expect_type!(self, String, sep);
        Ok(Value::List(Arc::new(RainList(
            s.split(sep.as_str())
                .map(|s| Value::String(Arc::new(s.to_owned())))
                .collect(),
        ))))
    }

    fn index(self) -> Result<Value> {
        *self.cache_hint = true;
        let ((indexable_nid, indexable_value), (index_nid, index_value)) = unpack_args!(self, 2);
        match index_value {
            Value::Integer(index) => {
                let list = expect_type!(self, List, (indexable_nid, indexable_value));
                let Ok(i) = usize::try_from(&index.0) else {
                    return Ok(Value::Unit);
                };
                list.0.get(i).cloned().ok_or_else(|| {
                    self.caller_cx.nid_err(
                        self.nid,
                        RunnerError::IndexOutOfBounds(index.as_ref().clone()),
                    )
                })
            }
            Value::String(name) => {
                let Some(v) = self.runner.evaluate_named_index(
                    self.caller_cx,
                    indexable_value,
                    self.call_span,
                    name.as_str(),
                )?
                else {
                    return Err(self.caller_cx.nid_err(
                        self.nid,
                        RunnerError::IndexKeyNotFound(name.as_str().to_owned()),
                    ));
                };
                Ok(v)
            }
            _ => Err(self.caller_cx.nid_err(
                index_nid,
                RunnerError::ExpectedType {
                    actual: indexable_value.rain_type_id(),
                    expected: Cow::Borrowed(&[RainTypeId::String, RainTypeId::Integer]),
                },
            )),
        }
    }

    fn host_info(self) -> Result<Value> {
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

    fn string_contains(self) -> Result<Value> {
        let (haystack, needle) = unpack_args!(self, 2);
        let haystack = expect_type!(self, String, haystack);
        let needle = expect_type!(self, String, needle);
        Ok(Value::Boolean(haystack.contains(&**needle)))
    }

    fn string_replace_all(self) -> Result<Value> {
        let (haystack, needle, replacement) = unpack_args!(self, 3);
        let haystack = expect_type!(self, String, haystack);
        let needle = expect_type!(self, String, needle);
        let replacement = expect_type!(self, String, replacement);
        Ok(Value::String(Arc::new(
            haystack.replace(&**needle, replacement),
        )))
    }

    fn regex_replace_f(self) -> Result<Value> {
        let (haystack, pattern, (func_nid, func_value)) = unpack_args!(self, 3);
        let haystack = expect_type!(self, String, haystack);
        let pattern = expect_type!(self, String, pattern);
        let re = regex::Regex::new(pattern).map_err(|err| {
            self.caller_cx
                .nid_err(self.nid, RunnerError::InvalidRegex(err))
        })?;
        let mut err_acc = None;
        let out = re.replace_all(haystack, |cap: &regex::Captures<'_>| -> String {
            let matched = Value::String(Arc::new(cap.get_match().as_str().to_owned()));
            let res = self.runner.call_function_like(
                self.caller_cx,
                self.nid,
                func_value,
                self.call_span,
                vec![(func_nid, matched)],
            );
            let replacement = match res {
                Ok(replacement) => replacement,
                Err(err) => {
                    if err_acc.is_none() {
                        err_acc = Some(err);
                    }
                    return String::new();
                }
            };

            let Value::String(v) = &replacement else {
                if err_acc.is_none() {
                    err_acc = Some(self.caller_cx.nid_err(
                        self.nid,
                        crate::runner::RunnerError::ExpectedType {
                            actual: replacement.rain_type_id(),
                            expected: std::borrow::Cow::Borrowed(&[
                                crate::runner::value::RainTypeId::String,
                            ]),
                        },
                    ));
                }
                return String::new();
            };
            debug_assert_eq!(
                replacement.rain_type_id(),
                crate::runner::value::RainTypeId::String
            );
            v.to_string()
        });
        if let Some(err) = err_acc {
            return Err(err);
        }
        Ok(Value::String(Arc::new(out.into_owned())))
    }

    fn stringify(self) -> Result<Value> {
        let (nid, value) = unpack_args!(self, 1);
        Ok(Value::String(Arc::new(self.runner.stringify_value(
            self.caller_cx,
            nid,
            value,
        )?)))
    }

    fn embed(self) -> Result<Value> {
        *self.cache_hint = false;
        self.no_args()?;
        let cache_key = CacheKey::Embed;
        if let Some(v) = self.runner.cache.get_value(
            &cache_key,
            self.runner.driver,
            &mut self.runner.local_file_hash_cache,
        ) {
            return Ok(v);
        }
        let start = Instant::now();
        let Some(src) = self.runner.driver.embed_src() else {
            return Err(self.caller_cx.nid_err(self.nid, RunnerError::NoEmbed));
        };
        let module = Module::parse(src.as_ref());
        let id = self
            .runner
            .ir
            .insert_module(None, src, module)
            .map_err(|err| err.convert().with_trace(self.caller_cx.stacktrace.clone()))?;
        let v = Value::Module(id);
        self.runner.cache.put(
            cache_key,
            CacheEntry {
                execution_time: start.elapsed(),
                expires: None,
                etag: None,
                deps: DepList::new(),
                value: v.clone(),
            },
        );
        Ok(v)
    }

    fn rust_eq(self) -> Result<Value> {
        *self.cache_hint = false;
        let ((_, a), (_, b)) = unpack_args!(self, 2);
        Ok(Value::Boolean(a == b))
    }

    fn get_secret(self) -> Result<Value> {
        let name = expect_type!(self, String, unpack_args!(self, 1));
        self.deps.push(Dep::Secret);
        let secret = self
            .runner
            .driver
            .get_secret(name)
            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
        Ok(Value::String(Arc::new(secret)))
    }

    fn set_cache_never(self) -> Result<Value> {
        self.no_args()?;
        self.deps.push(Dep::Uncacheable);
        self.deps.push(Dep::MutateDeps);
        Ok(Value::Unit)
    }

    fn clear_calling_cache_deps(self) -> Result<Value> {
        self.no_args()?;
        debug!("cleared deps {:?}", self.caller_cx.deps);
        self.caller_cx.deps.clear();
        self.deps.push(Dep::MutateDeps);
        Ok(Value::Unit)
    }

    fn merge_records(self) -> Result<Value> {
        let (record1, record2) = unpack_args!(self, 2);
        let record1 = expect_type!(self, Record, record1);
        let record2 = expect_type!(self, Record, record2);
        let mut out_record = record1.as_ref().clone();
        for (k, v) in &record2.as_ref().0 {
            out_record.0.insert(k.clone(), v.clone());
        }
        Ok(Value::Record(Arc::new(out_record)))
    }

    fn parse_target_triple(self) -> Result<Value> {
        let triple = expect_type!(self, String, unpack_args!(self, 1));
        let triple = match target_lexicon::Triple::from_str(triple) {
            Ok(triple) => triple,
            Err(err) => {
                return Err(self.caller_cx.nid_err(
                    self.nid,
                    RunnerError::Makeshift(Cow::Owned(err.to_string())),
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

    fn git_contents(self) -> Result<Value> {
        let (url, commit) = unpack_args!(self, 2);
        let url = expect_type!(self, String, url);
        let commit = expect_type!(self, String, commit);
        let area = self
            .runner
            .driver
            .git_contents(url, commit)
            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
        Ok(area.to_value())
    }

    fn git_lfs_smudge(self) -> Result<Value> {
        let area = self.expect_fs_area(unpack_args!(self, 1))?;
        let new_area = self
            .runner
            .driver
            .git_lfs_smudge(&area)
            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
        Ok(new_area.to_value())
    }

    fn env_var(self) -> Result<Value> {
        self.deps.push(Dep::EnvVar);
        let var_name = expect_type!(self, String, unpack_args!(self, 1));
        if let Some(value) = self
            .runner
            .driver
            .env_var(var_name)
            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?
        {
            Ok(Value::String(Arc::new(value)))
        } else {
            Ok(Value::Unit)
        }
    }

    fn copy_file(mut self) -> Result<Value> {
        self.add_deps_from_args();
        let (file, name, executable) = unpack_args!(self, 3);
        let file = self.expect_file(file)?;
        let name = expect_type!(self, String, name);
        let executable = expect_type!(self, Boolean, executable);
        let new_file = self
            .runner
            .driver
            .copy_file(&file, name, *executable)
            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
        Ok(Value::GeneratedFile(Arc::new(new_file)))
    }

    fn escape_hard(self) -> Result<Value> {
        self.deps.push(Dep::Escape);
        let file_path = expect_type!(self, String, unpack_args!(self, 1));
        Ok(Value::EscapeFile(Arc::new(
            AbsolutePathBuf::try_from(Path::new(file_path.as_str())).map_err(|err| {
                self.caller_cx.nid_err(
                    self.nid,
                    RunnerError::MakeshiftIO("absolute path".into(), err),
                )
            })?,
        )))
    }

    fn get_type(self) -> Result<Value> {
        *self.cache_hint = false;
        let (_arg_nid, arg_value) = unpack_args!(self, 1);
        Ok(Value::Type(arg_value.rain_type_id()))
    }

    fn fold(self) -> Result<Value> {
        let ((initial_nid, initial_value), list, (func_nid, func_value)) = unpack_args!(self, 3);
        let list = expect_type!(self, List, list);
        let mut acc = initial_value.clone();
        for item in list.0.clone() {
            acc = self.runner.call_function_like(
                self.caller_cx,
                self.nid,
                func_value,
                self.call_span,
                vec![(initial_nid, acc), (func_nid, item)],
            )?;
        }
        Ok(acc)
    }

    fn record_keys(self) -> Result<Value> {
        let record = expect_type!(self, Record, unpack_args!(self, 1));
        Ok(Value::List(Arc::new(RainList(
            record
                .0
                .keys()
                .map(|k| Value::String(Arc::new(k.clone())))
                .collect(),
        ))))
    }

    fn config(self) -> Result<Value> {
        let name = expect_type!(self, String, unpack_args!(self, 1));
        match self.runner.driver.config(name.as_str()) {
            Some(v) => Ok(Value::String(v)),
            None => Ok(Value::Unit),
        }
    }

    fn concrete_types(self) -> Result<Value> {
        self.no_args()?;
        let mut map = IndexMap::new();
        map.insert(
            String::from("local_file"),
            Value::Type(RainTypeId::LocalFile),
        );
        map.insert(String::from("local_dir"), Value::Type(RainTypeId::LocalDir));
        map.insert(
            String::from("generated_file"),
            Value::Type(RainTypeId::GeneratedFile),
        );
        map.insert(
            String::from("generated_dir"),
            Value::Type(RainTypeId::GeneratedDir),
        );
        map.insert(
            String::from("local_fs_area"),
            Value::Type(RainTypeId::LocalFSArea),
        );
        map.insert(
            String::from("generated_fs_area"),
            Value::Type(RainTypeId::GeneratedFSArea),
        );
        Ok(Value::Record(Arc::new(RainRecord(map))))
    }

    fn inc_counter(self) -> Result<Value> {
        self.deps.push(Dep::Counter);
        let name = expect_type!(self, String, unpack_args!(self, 1));
        self.runner.driver.increment_counter(name.alias());
        Ok(Value::Unit)
    }

    fn try_function(self) -> Result<Value> {
        let (_, func_value) = unpack_args!(self, 1);
        let result = self.runner.call_function_like(
            self.caller_cx,
            self.nid,
            func_value,
            self.call_span,
            vec![],
        );
        let mut out = IndexMap::<String, Value>::new();
        match result {
            Ok(v) => {
                out.insert("success".to_owned(), Value::Boolean(true));
                out.insert("value".to_owned(), v);
            }
            Err(err) => {
                out.insert("success".to_owned(), Value::Boolean(false));
                out.insert(
                    "error".to_owned(),
                    Value::String(Arc::new(err.err_span.err.to_string())),
                );
            }
        }
        Ok(Value::Record(Arc::new(RainRecord(out))))
    }

    fn create_unique(self) -> Result<Value> {
        let v = self.runner.next_unique.fetch_add(1, Ordering::Relaxed);
        Ok(Value::Unique(v))
    }

    fn offline(self) -> Result<Value> {
        Ok(Value::Boolean(self.runner.offline))
    }

    fn git_describe(self) -> Result<Value> {
        let area = self.expect_fs_area(unpack_args!(self, 1))?;
        let describe = match &area {
            FSArea::Local(absolute_path_buf) => self
                .runner
                .driver
                .git_describe(absolute_path_buf)
                .map_err(|err| self.caller_cx.nid_err(self.nid, err))?,
            FSArea::Generated(generated_fsarea) => generated_fsarea.git_describe.clone(),
        };
        let mut out = IndexMap::<String, Value>::new();
        if let Some(exists) = describe {
            out.insert("exists".into(), Value::Boolean(true));
            out.insert("commit".into(), Value::String(Arc::new(exists.commit)));
            out.insert("dirty".into(), Value::Boolean(exists.dirty));
        } else {
            out.insert("exists".into(), Value::Boolean(false));
        }
        Ok(Value::Record(Arc::new(RainRecord(out))))
    }
}
