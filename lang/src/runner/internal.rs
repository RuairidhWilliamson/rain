#![allow(clippy::unnecessary_wraps)]

mod archives;
mod download;
mod fs;
mod macros;
mod run;

use std::{
    borrow::Cow,
    hash::Hash,
    ops::RangeInclusive,
    path::{Path, PathBuf},
    str::FromStr as _,
    sync::Arc,
    time::Instant,
};

use indexmap::IndexMap;
use num_bigint::BigInt;

use crate::{
    afs::{Dir, File, absolute::AbsolutePathBuf, area::FSArea},
    ast::{Module, NodeId},
    driver::DriverTrait,
    local_span::LocalSpan,
    runner::{
        Result, ResultValue,
        cache::{CacheEntry, CacheKey, CacheTrait},
        cx::Cx,
        dep::Dep,
        dep_list::DepList,
        error::{RunnerError, Throwing},
        internal::macros::{expect_type, single_arg, three_args, two_args},
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
    Stringify,
    Throw,
    Unit,
    FileName,
    CopyDir,
    Config,
    ConcreteTypes,
    IncCounter,
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
            "_stringify" => Some(Self::Stringify),
            "_throw" => Some(Self::Throw),
            "_unit" => Some(Self::Unit),
            "_file_name" => Some(Self::FileName),
            "_copy_dir" => Some(Self::CopyDir),
            "_config" => Some(Self::Config),
            "_concrete_types" => Some(Self::ConcreteTypes),
            "_inc_counter" => Some(Self::IncCounter),
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
    pub fn call_internal_function(self) -> ResultValue {
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

    fn import(mut self) -> ResultValue {
        self.add_deps_from_args();
        *self.cache_hint = false;
        let f = self.expect_file(single_arg!(self))?;
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

    fn module_file(self) -> ResultValue {
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

    fn throw(self) -> ResultValue {
        *self.cache_hint = false;
        let (_, err_value) = single_arg!(self);
        Err(self
            .caller_cx
            .module
            .span(self.nid)
            .with_module(self.caller_cx.module.id)
            .with_error(Throwing::Recoverable(err_value.clone()))
            .with_trace(self.caller_cx.stacktrace.clone()))
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
            self.caller_cx.nid_err(
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
            self.caller_cx
                .nid_err(self.nid, RunnerError::Makeshift(err.to_string().into()))
        })?;
        Ok(json_to_rain(parsed))
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
        *self.cache_hint = true;
        let ((indexable_nid, indexable_value), (index_nid, index_value)) = two_args!(self);
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

    fn string_replace_all(self) -> ResultValue {
        let (haystack, needle, replacement) = three_args!(self);
        let haystack = expect_type!(self, String, haystack);
        let needle = expect_type!(self, String, needle);
        let replacement = expect_type!(self, String, replacement);
        Ok(Value::String(Arc::new(
            haystack.replace(&**needle, replacement),
        )))
    }

    fn stringify_impl(&self, nid: NodeId, v: &Value) -> Result<String> {
        match v {
            Value::String(s) => Ok(s.as_ref().clone()),
            Value::GeneratedFile(f) => Ok(self
                .runner
                .driver
                .resolve_fs_entry(f.fsinner().into())
                .display()
                .to_string()),
            Value::LocalFile(f) => Ok(self
                .runner
                .driver
                .resolve_fs_entry(f.fsinner().into())
                .display()
                .to_string()),
            Value::GeneratedFSArea(area) => Ok(self
                .runner
                .driver
                .resolve_fs_entry(Dir::root(area.as_ref().into()).fsinner())
                .display()
                .to_string()),
            Value::LocalFSArea(area) => Ok(self
                .runner
                .driver
                .resolve_fs_entry(Dir::root(area.as_ref().into()).fsinner())
                .display()
                .to_string()),
            Value::GeneratedDir(d) => Ok(self
                .runner
                .driver
                .resolve_fs_entry(d.fsinner().into())
                .display()
                .to_string()),
            Value::LocalDir(d) => Ok(self
                .runner
                .driver
                .resolve_fs_entry(d.fsinner().into())
                .display()
                .to_string()),
            Value::EscapeFile(f) => Ok(format!("{}", f.0.display())),
            Value::Integer(i) => Ok(i.to_string()),
            Value::Boolean(b) => Ok(b.to_string()),
            _ => Err(self.caller_cx.nid_err(
                nid,
                RunnerError::ExpectedType {
                    actual: v.rain_type_id(),
                    expected: Cow::Borrowed(&[
                        RainTypeId::String,
                        RainTypeId::GeneratedFile,
                        RainTypeId::GeneratedDir,
                        RainTypeId::GeneratedFSArea,
                        RainTypeId::LocalFSArea,
                        RainTypeId::LocalFile,
                        RainTypeId::LocalDir,
                        RainTypeId::EscapeFile,
                        RainTypeId::Integer,
                        RainTypeId::Boolean,
                    ]),
                },
            )),
        }
    }

    fn stringify(self) -> ResultValue {
        let (nid, value) = single_arg!(self);
        Ok(Value::String(Arc::new(self.stringify_impl(nid, value)?)))
    }

    fn embed(self) -> ResultValue {
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
            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
        Ok(Value::String(Arc::new(secret)))
    }

    fn set_cache_never(self) -> ResultValue {
        self.no_args()?;
        self.deps.push(Dep::Uncacheable);
        self.deps.push(Dep::MutateDeps);
        Ok(Value::Unit)
    }

    fn clear_calling_cache_deps(self) -> ResultValue {
        self.no_args()?;
        log::debug!("cleared deps {:?}", self.caller_cx.deps);
        self.caller_cx.deps.clear();
        self.deps.push(Dep::MutateDeps);
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

    fn git_contents(self) -> ResultValue {
        let (url, commit) = two_args!(self);
        let url = expect_type!(self, String, url);
        let commit = expect_type!(self, String, commit);
        let area = self
            .runner
            .driver
            .git_contents(url, commit)
            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
        Ok(area.to_value())
    }

    fn git_lfs_smudge(self) -> ResultValue {
        let area = self.expect_fs_area(single_arg!(self))?;
        let new_area = self
            .runner
            .driver
            .git_lfs_smudge(&area)
            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
        Ok(new_area.to_value())
    }

    fn env_var(self) -> ResultValue {
        self.deps.push(Dep::EnvVar);
        let var_name = expect_type!(self, String, single_arg!(self));
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

    fn copy_file(mut self) -> ResultValue {
        self.add_deps_from_args();
        let (file, name, executable) = three_args!(self);
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

    fn escape_hard(self) -> ResultValue {
        self.deps.push(Dep::Escape);
        let file_path = expect_type!(self, String, single_arg!(self));
        Ok(Value::EscapeFile(Arc::new(
            AbsolutePathBuf::try_from(Path::new(file_path.as_str())).map_err(|err| {
                self.caller_cx.nid_err(
                    self.nid,
                    RunnerError::MakeshiftIO("absolute path".into(), err),
                )
            })?,
        )))
    }

    fn get_type(self) -> ResultValue {
        *self.cache_hint = false;
        let (_arg_nid, arg_value) = single_arg!(self);
        Ok(Value::Type(arg_value.rain_type_id()))
    }

    fn fold(self) -> ResultValue {
        let ((initial_nid, initial_value), list, (func_nid, func_value)) = three_args!(self);
        let list = expect_type!(self, List, list);
        let mut acc = initial_value.clone();
        for item in list.0.clone() {
            acc = self.runner.call_function(
                self.caller_cx,
                self.nid,
                func_value,
                self.call_span,
                vec![(initial_nid, acc), (func_nid, item)],
            )?;
        }
        Ok(acc)
    }

    fn record_keys(self) -> ResultValue {
        let record = expect_type!(self, Record, single_arg!(self));
        Ok(Value::List(Arc::new(RainList(
            record
                .0
                .keys()
                .map(|k| Value::String(Arc::new(k.clone())))
                .collect(),
        ))))
    }

    fn config(self) -> ResultValue {
        let name = expect_type!(self, String, single_arg!(self));
        match self.runner.driver.config(name.as_str()) {
            Some(v) => Ok(Value::String(v)),
            None => Ok(Value::Unit),
        }
    }

    fn concrete_types(self) -> ResultValue {
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

    fn inc_counter(self) -> ResultValue {
        self.deps.push(Dep::Counter);
        let name = expect_type!(self, String, single_arg!(self));
        self.runner.driver.increment_counter(Arc::clone(name));
        Ok(Value::Unit)
    }
}
