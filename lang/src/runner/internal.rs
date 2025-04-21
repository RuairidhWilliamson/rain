#![allow(clippy::unnecessary_wraps, clippy::needless_pass_by_value)]

use std::{collections::HashMap, ops::RangeInclusive, sync::Arc, time::Instant};

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
        path::FilePath,
    },
    ast::{FnCall, Node, NodeId},
    driver::{DownloadStatus, DriverTrait, FSEntryQueryResult, RunOptions},
    span::ErrorSpan,
};

use super::{
    Cx, Result, ResultValue,
    cache::{CacheEntry, CacheKey},
    dep::Dep,
    error::RunnerError,
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
    CreateArea,
    ReadFile,
    CreateFile,
    LocalArea,
    SplitString,
    Index,
    HostInfo,
    StringContains,
    ExportToLocal,
    FileMetadata,
    Glob,
    Foreach,
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
            "_file_metadata" => Some(Self::FileMetadata),
            "_glob" => Some(Self::Glob),
            "_foreach" => Some(Self::Foreach),
            _ => None,
        }
    }

    pub fn call_internal_function<D: DriverTrait>(self, icx: InternalCx<D>) -> ResultValue {
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
            Self::FileMetadata => icx.file_metadata(),
            Self::Glob => icx.glob(),
            Self::Foreach => icx.foreach(),
        }
    }
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

fn enter_call(driver: &dyn DriverTrait, s: String) -> Call {
    driver.enter_call(&s);
    Call { driver, s }
}

// TODO: Cleanup all those lifetimes :o
pub struct InternalCx<'a, 'b, 'c, 'd, 'e, D> {
    pub func: InternalFunction,
    pub runner: &'a mut super::Runner<'e, D>,
    pub cx: &'c mut Cx<'b>,
    pub nid: NodeId,
    pub fn_call: &'d FnCall,
    pub arg_values: Vec<(NodeId, Value)>,
}

impl<D: DriverTrait> InternalCx<'_, '_, '_, '_, '_, D> {
    fn single_arg(&self) -> Result<(&NodeId, &Value)> {
        match &self.arg_values[..] {
            [(arg_nid, arg_value)] => Ok((arg_nid, arg_value)),
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

    fn cache(&self, f: impl FnOnce() -> ResultValue) -> ResultValue {
        let cache_key = CacheKey::InternalFunction {
            func: self.func,
            args: self.arg_values.iter().map(|(_, v)| v.clone()).collect(),
        };
        if let Some(v) = self.runner.cache.get_value(&cache_key) {
            return Ok(v);
        }
        let start = Instant::now();
        let v = f()?;
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

    fn print(self) -> ResultValue {
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
                self.cx.deps.push(Dep::Uncacheable);
                let Value::String(relative_path) = relative_path_value else {
                    return Err(self.cx.nid_err(
                        *relative_path_nid,
                        RunnerError::ExpectedType {
                            actual: relative_path_value.rain_type_id(),
                            expected: &[RainTypeId::String],
                        },
                    ));
                };
                let file_path = self
                    .cx
                    .module
                    .file
                    .path()
                    .parent()
                    .ok_or_else(|| {
                        self.cx
                            .nid_err(self.nid, PathError::NoParentDirectory.into())
                    })?
                    .join(relative_path)
                    .map_err(|err| self.cx.nid_err(*relative_path_nid, err.into()))?;
                Ok(FSEntry {
                    area: self.cx.module.file.area().clone(),
                    path: file_path,
                })
            }
            [(parent_nid, parent_value), (path_nid, path_value)] => {
                self.cx.deps.push(Dep::Uncacheable);
                let Value::String(path) = path_value else {
                    return Err(self.cx.nid_err(
                        *path_nid,
                        RunnerError::ExpectedType {
                            actual: path_value.rain_type_id(),
                            expected: &[RainTypeId::String],
                        },
                    ));
                };
                match parent_value {
                    Value::FileArea(area) => {
                        let file_path = FilePath::new(path)
                            .map_err(|err| self.cx.nid_err(*path_nid, err.into()))?;
                        Ok(FSEntry {
                            area: area.as_ref().clone(),
                            path: file_path,
                        })
                    }
                    Value::Dir(dir) => {
                        let area = dir.area();
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
            _ => {
                let required = 1..=2;
                Err(self.cx.err(
                    self.fn_call.rparen_token,
                    RunnerError::IncorrectArgs {
                        required,
                        actual: self.arg_values.len(),
                    },
                ))
            }
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
        let (file_nid, file_value) = self.single_arg()?;
        let Value::File(f) = file_value else {
            return Err(self.cx.nid_err(
                *file_nid,
                RunnerError::ExpectedType {
                    actual: file_value.rain_type_id(),
                    expected: &[RainTypeId::File],
                },
            ));
        };
        let src = self
            .runner
            .driver
            .read_file(f)
            .map_err(|err| self.cx.nid_err(self.nid, RunnerError::ImportIOError(err)))?;
        let module = crate::ast::parser::parse_module(&src);
        let id = self
            .runner
            .ir
            .insert_module(f.as_ref().clone(), src, module)
            .map_err(ErrorSpan::convert)?;
        Ok(Value::Module(id))
    }

    fn module_file(self) -> ResultValue {
        self.no_args()?;
        Ok(Value::File(Arc::new(self.cx.module.file.clone())))
    }

    fn extract_zip(self) -> ResultValue {
        let (file_nid, file_value) = self.single_arg()?;
        let Value::File(f) = file_value else {
            return Err(self.cx.nid_err(
                *file_nid,
                RunnerError::ExpectedType {
                    actual: file_value.rain_type_id(),
                    expected: &[RainTypeId::File],
                },
            ));
        };
        self.cache(|| {
            let area = self
                .runner
                .driver
                .extract_zip(f)
                .map_err(|err| self.cx.nid_err(self.nid, err))?;
            Ok(Value::FileArea(Arc::new(area)))
        })
    }

    fn extract_tar_gz(self) -> ResultValue {
        let (file_nid, file_value) = self.single_arg()?;
        let Value::File(f) = file_value else {
            return Err(self.cx.nid_err(
                *file_nid,
                RunnerError::ExpectedType {
                    actual: file_value.rain_type_id(),
                    expected: &[RainTypeId::File],
                },
            ));
        };
        self.cache(|| {
            let area = self
                .runner
                .driver
                .extract_tar_gz(f)
                .map_err(|err| self.cx.nid_err(self.nid, err))?;
            Ok(Value::FileArea(Arc::new(area)))
        })
    }

    fn extract_tar_xz(self) -> ResultValue {
        let (file_nid, file_value) = self.single_arg()?;
        let Value::File(f) = file_value else {
            return Err(self.cx.nid_err(
                *file_nid,
                RunnerError::ExpectedType {
                    actual: file_value.rain_type_id(),
                    expected: &[RainTypeId::File],
                },
            ));
        };
        self.cache(|| {
            let area = self
                .runner
                .driver
                .extract_tar_xz(f)
                .map_err(|err| self.cx.nid_err(self.nid, err))?;
            Ok(Value::FileArea(Arc::new(area)))
        })
    }

    fn run(self) -> ResultValue {
        match &self.arg_values[..] {
            [
                (area_nid, area_value),
                (file_nid, file_value),
                (args_nid, args_value),
                (env_nid, env_value),
            ] => {
                let overlay_area = match area_value {
                    Value::Unit => None,
                    Value::FileArea(area) => Some(area.as_ref()),
                    _ => Err(self.cx.nid_err(
                        *area_nid,
                        RunnerError::ExpectedType {
                            actual: area_value.rain_type_id(),
                            expected: &[RainTypeId::FileArea],
                        },
                    ))?,
                };
                let Value::File(file) = file_value else {
                    return Err(self.cx.nid_err(
                        *file_nid,
                        RunnerError::ExpectedType {
                            actual: file_value.rain_type_id(),
                            expected: &[RainTypeId::File],
                        },
                    ));
                };
                let Value::List(args) = args_value else {
                    return Err(self.cx.nid_err(
                        *args_nid,
                        RunnerError::ExpectedType {
                            actual: args_value.rain_type_id(),
                            expected: &[RainTypeId::List],
                        },
                    ));
                };
                let args = args
                    .0
                    .iter()
                    .map(|value| self.stringify_args(*args_nid, value))
                    .collect::<Result<Vec<String>>>()?;
                let Value::Record(env) = env_value else {
                    return Err(self.cx.nid_err(
                        *env_nid,
                        RunnerError::ExpectedType {
                            actual: env_value.rain_type_id(),
                            expected: &[RainTypeId::List],
                        },
                    ));
                };
                let env = env
                    .0
                    .iter()
                    .map(|(key, value)| self.stringify_env(*env_nid, key, value))
                    .collect::<Result<HashMap<String, String>>>()?;
                let status = self
                    .runner
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
                    .map_err(|err| self.cx.nid_err(self.nid, err))?;
                let mut m = IndexMap::new();
                m.insert("success".to_owned(), Value::Boolean(status.success));
                m.insert(
                    "exit_code".to_owned(),
                    Value::Integer(Arc::new(RainInteger(status.exit_code.unwrap_or(-1).into()))),
                );
                m.insert("area".to_owned(), Value::FileArea(Arc::new(status.area)));
                m.insert("stdout".to_owned(), Value::String(Arc::new(status.stdout)));
                m.insert("stderr".to_owned(), Value::String(Arc::new(status.stderr)));
                Ok(Value::Record(Arc::new(RainRecord(m))))
            }
            _ => self.incorrect_args(4..=4),
        }
    }

    fn stringify_env(
        &self,
        env_nid: NodeId,
        key: &String,
        value: &Value,
    ) -> Result<(String, String)> {
        match value {
            Value::String(s) => Ok((key.to_owned(), s.to_string())),
            Value::File(f) => Ok((
                key.to_owned(),
                self.runner
                    .driver
                    .resolve_fs_entry(f.inner())
                    .display()
                    .to_string(),
            )),
            Value::Dir(d) => Ok((
                key.to_owned(),
                self.runner
                    .driver
                    .resolve_fs_entry(d.inner())
                    .display()
                    .to_string(),
            )),
            Value::FileArea(a) => Ok((
                key.to_owned(),
                self.runner
                    .driver
                    .resolve_fs_entry(Dir::root(a.as_ref().clone()).inner())
                    .display()
                    .to_string(),
            )),
            _ => Err(self.cx.nid_err(
                env_nid,
                RunnerError::ExpectedType {
                    actual: value.rain_type_id(),
                    expected: &[
                        RainTypeId::String,
                        RainTypeId::File,
                        RainTypeId::Dir,
                        RainTypeId::FileArea,
                    ],
                },
            )),
        }
    }

    fn stringify_args(&self, args_nid: NodeId, value: &Value) -> Result<String> {
        match value {
            Value::String(s) => Ok(s.to_string()),
            Value::File(f) => Ok(self
                .runner
                .driver
                .resolve_fs_entry(f.inner())
                .display()
                .to_string()),
            Value::Dir(d) => Ok(self
                .runner
                .driver
                .resolve_fs_entry(d.inner())
                .display()
                .to_string()),
            Value::FileArea(a) => Ok(self
                .runner
                .driver
                .resolve_fs_entry(Dir::root(a.as_ref().clone()).inner())
                .display()
                .to_string()),
            _ => Err(self.cx.nid_err(
                args_nid,
                RunnerError::ExpectedType {
                    actual: value.rain_type_id(),
                    expected: &[
                        RainTypeId::String,
                        RainTypeId::File,
                        RainTypeId::Dir,
                        RainTypeId::FileArea,
                    ],
                },
            )),
        }
    }

    fn escape_bin(self) -> ResultValue {
        let (name_nid, name_value) = self.single_arg()?;
        let Value::String(name) = name_value else {
            return Err(self.cx.nid_err(
                *name_nid,
                RunnerError::ExpectedType {
                    actual: name_value.rain_type_id(),
                    expected: &[RainTypeId::String],
                },
            ));
        };
        let path = self.runner.driver.escape_bin(name).ok_or_else(|| {
            self.cx.nid_err(
                self.nid,
                RunnerError::Makeshift("could not find bin".into()),
            )
        })?;
        let path = FilePath::new(&path.to_string_lossy())
            .map_err(|err| self.cx.nid_err(self.nid, RunnerError::PathError(err)))?;
        let entry = FSEntry {
            area: FileArea::Escape,
            path,
        };
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

    fn unit(self) -> ResultValue {
        self.no_args()?;
        Ok(Value::Unit)
    }

    fn get_area(self) -> ResultValue {
        let (file_nid, file_value) = self.single_arg()?;
        let Value::File(f) = file_value else {
            return Err(self.cx.nid_err(
                *file_nid,
                RunnerError::ExpectedType {
                    actual: file_value.rain_type_id(),
                    expected: &[RainTypeId::File],
                },
            ));
        };
        Ok(Value::FileArea(Arc::new(f.area().clone())))
    }

    fn download(self) -> ResultValue {
        match &self.arg_values[..] {
            [(url_nid, url_value), (name_nid, name_value)] => {
                let start = Instant::now();
                let Value::String(url) = url_value else {
                    return Err(self.cx.nid_err(
                        *url_nid,
                        RunnerError::ExpectedType {
                            actual: url_value.rain_type_id(),
                            expected: &[RainTypeId::String],
                        },
                    ));
                };
                let Value::String(name) = name_value else {
                    return Err(self.cx.nid_err(
                        *name_nid,
                        RunnerError::ExpectedType {
                            actual: name_value.rain_type_id(),
                            expected: &[RainTypeId::String],
                        },
                    ));
                };
                let cache_key = CacheKey::Download {
                    url: url.to_string(),
                };
                let call_description = format!("Download {url}");
                let _call = enter_call(self.runner.driver, call_description);
                let cache_entry = self.runner.cache.get(&cache_key);
                let etag: Option<&str> = cache_entry.as_ref().and_then(|e| e.etag.as_deref());
                let DownloadStatus {
                    ok,
                    status_code,
                    file,
                    etag,
                } = self
                    .runner
                    .driver
                    .download(url, name, etag)
                    .map_err(|err| self.cx.nid_err(self.nid, err))?;
                if !ok && status_code == Some(304) {
                    // Etag matched we can use our cached value!
                    if let Some(cache_entry) = cache_entry {
                        return Ok(cache_entry.value);
                    }
                }
                let mut m = IndexMap::new();
                m.insert("ok".to_owned(), Value::Boolean(ok));
                m.insert(
                    "status_code".to_owned(),
                    Value::Integer(Arc::new(RainInteger(
                        status_code.unwrap_or_default().into(),
                    ))),
                );
                if let Some(file) = file {
                    m.insert("file".to_owned(), Value::File(Arc::new(file)));
                } else {
                    m.insert("file".to_owned(), Value::Unit);
                }
                let out = Value::Record(Arc::new(RainRecord(m)));
                self.runner.cache.put(
                    cache_key,
                    CacheEntry {
                        execution_time: start.elapsed(),
                        etag,
                        expires: None,
                        deps: Vec::new(),
                        value: out.clone(),
                    },
                );
                Ok(out)
            }
            _ => self.incorrect_args(2..=2),
        }
    }

    fn throw(self) -> ResultValue {
        match &self.arg_values[..] {
            [(_, err_value)] => Err(self
                .cx
                .module
                .span(self.nid)
                .with_module(self.cx.module.id)
                .with_error(super::error::Throwing::Recoverable(err_value.clone()))),
            _ => self.incorrect_args(1..=1),
        }
    }

    fn sha256(self) -> ResultValue {
        let (file_nid, file_value) = self.single_arg()?;
        let Value::File(f) = file_value else {
            return Err(self.cx.nid_err(
                *file_nid,
                RunnerError::ExpectedType {
                    actual: file_value.rain_type_id(),
                    expected: &[RainTypeId::File],
                },
            ));
        };
        self.cache(|| {
            Ok(Value::String(Arc::new(
                self.runner
                    .driver
                    .sha256(f)
                    .map_err(|err| self.cx.nid_err(self.nid, err))?,
            )))
        })
    }

    fn sha512(self) -> ResultValue {
        let (file_nid, file_value) = self.single_arg()?;
        let Value::File(f) = file_value else {
            return Err(self.cx.nid_err(
                *file_nid,
                RunnerError::ExpectedType {
                    actual: file_value.rain_type_id(),
                    expected: &[RainTypeId::File],
                },
            ));
        };
        self.cache(|| {
            Ok(Value::String(Arc::new(
                self.runner
                    .driver
                    .sha512(f)
                    .map_err(|err| self.cx.nid_err(self.nid, err))?,
            )))
        })
    }

    #[expect(clippy::unwrap_used)]
    fn bytes_to_string(self) -> ResultValue {
        let (bytes_nid, bytes_value) = self.single_arg()?;
        let Value::List(list) = bytes_value else {
            return Err(self.cx.nid_err(
                *bytes_nid,
                RunnerError::ExpectedType {
                    actual: bytes_value.rain_type_id(),
                    expected: &[RainTypeId::List],
                },
            ));
        };
        self.cache(|| {
            let bytes: Vec<u8> = list
                .0
                .iter()
                .map(|b| -> u8 {
                    let Value::Integer(b) = b else {
                        todo!("not an integer")
                    };
                    b.0.iter_u32_digits().next().unwrap().try_into().unwrap()
                })
                .collect();
            Ok(Value::String(Arc::new(String::from_utf8(bytes).unwrap())))
        })
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

        let (contents_nid, contents_value) = self.single_arg()?;
        let Value::String(contents) = contents_value else {
            return Err(self.cx.nid_err(
                *contents_nid,
                RunnerError::ExpectedType {
                    actual: contents_value.rain_type_id(),
                    expected: &[RainTypeId::String],
                },
            ));
        };
        self.cache(|| {
            let parsed: toml::Value = toml::de::from_str(contents).map_err(|err| {
                self.cx.nid_err(
                    self.nid,
                    RunnerError::Makeshift(err.message().to_owned().into()),
                )
            })?;
            Ok(toml_to_rain(parsed))
        })
    }

    fn create_area(self) -> ResultValue {
        let (dirs_nid, dirs_value) = self.single_arg()?;
        let Value::List(dirs) = dirs_value else {
            return Err(self.cx.nid_err(
                *dirs_nid,
                RunnerError::ExpectedType {
                    actual: dirs_value.rain_type_id(),
                    expected: &[RainTypeId::Dir],
                },
            ));
        };
        let dirs: Vec<&Dir> = dirs
            .0
            .iter()
            .map(|dir| {
                let Value::Dir(d) = dir else {
                    return Err(self.cx.nid_err(
                        *dirs_nid,
                        RunnerError::ExpectedType {
                            actual: dir.rain_type_id(),
                            expected: &[RainTypeId::Dir],
                        },
                    ));
                };
                Ok(d.as_ref())
            })
            .collect::<Result<Vec<&Dir>, _>>()?;
        let merged_area = self
            .runner
            .driver
            .create_area(&dirs)
            .map_err(|err| self.cx.nid_err(self.nid, err))?;
        Ok(Value::FileArea(Arc::new(merged_area)))
    }

    fn read_file(self) -> ResultValue {
        let (file_nid, file_value) = self.single_arg()?;
        let Value::File(f) = file_value else {
            return Err(self.cx.nid_err(
                *file_nid,
                RunnerError::ExpectedType {
                    actual: file_value.rain_type_id(),
                    expected: &[RainTypeId::File],
                },
            ));
        };
        Ok(Value::String(Arc::new(
            self.runner
                .driver
                .read_file(f)
                .map_err(|err| self.cx.nid_err(self.nid, RunnerError::AreaIOError(err)))?,
        )))
    }

    fn create_file(self) -> ResultValue {
        match &self.arg_values[..] {
            [(contents_nid, contents_value), (name_nid, name_value)] => {
                let Value::String(contents) = contents_value else {
                    return Err(self.cx.nid_err(
                        *contents_nid,
                        RunnerError::ExpectedType {
                            actual: contents_value.rain_type_id(),
                            expected: &[RainTypeId::String],
                        },
                    ));
                };
                let Value::String(name) = name_value else {
                    return Err(self.cx.nid_err(
                        *name_nid,
                        RunnerError::ExpectedType {
                            actual: name_value.rain_type_id(),
                            expected: &[RainTypeId::String],
                        },
                    ));
                };
                Ok(Value::File(Arc::new(
                    self.runner
                        .driver
                        .create_file(contents, name)
                        .map_err(|err| self.cx.nid_err(self.nid, err))?,
                )))
            }
            _ => self.incorrect_args(2..=2),
        }
    }

    fn debug(mut self) -> ResultValue {
        if self.arg_values.len() != 1 {
            return self.incorrect_args(1..=1);
        }
        let Some((_nid, value)) = self.arg_values.pop() else {
            return self.incorrect_args(1..=1);
        };
        let p = if let Value::String(s) = &value {
            s.to_string()
        } else {
            format!("{value}")
        };
        self.runner.driver.print(p);
        Ok(value)
    }

    fn local_area(self) -> ResultValue {
        let FileArea::Local(current_area_path) = &self.cx.module.file.area() else {
            return Err(self.cx.nid_err(self.nid, RunnerError::IllegalLocalArea));
        };
        let (path_nid, path_value) = self.arg_values.first().ok_or_else(|| {
            self.cx.nid_err(
                self.nid,
                RunnerError::IncorrectArgs {
                    required: 1..=1,
                    actual: self.arg_values.len(),
                },
            )
        })?;
        let Value::String(path) = path_value else {
            return Err(self.cx.nid_err(
                *path_nid,
                RunnerError::ExpectedType {
                    actual: path_value.rain_type_id(),
                    expected: &[RainTypeId::String],
                },
            ));
        };
        let area_path = current_area_path.join(&**path);
        let area_path = AbsolutePathBuf::try_from(area_path.as_path())
            .map_err(|err| self.cx.nid_err(self.nid, RunnerError::AreaIOError(err)))?;
        let entry = FSEntry::new(FileArea::Local(area_path), FilePath::root());
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
        match &self.arg_values[..] {
            [(string_nid, string_value), (sep_nid, sep_value)] => {
                let Value::String(s) = string_value else {
                    return Err(self.cx.nid_err(
                        *string_nid,
                        RunnerError::ExpectedType {
                            actual: string_value.rain_type_id(),
                            expected: &[RainTypeId::String],
                        },
                    ));
                };
                let Value::String(sep) = sep_value else {
                    return Err(self.cx.nid_err(
                        *sep_nid,
                        RunnerError::ExpectedType {
                            actual: sep_value.rain_type_id(),
                            expected: &[RainTypeId::String],
                        },
                    ));
                };
                Ok(Value::List(Arc::new(RainList(
                    s.split(sep.as_str())
                        .map(|s| Value::String(Arc::new(s.to_owned())))
                        .collect(),
                ))))
            }
            _ => self.incorrect_args(2..=2),
        }
    }

    fn index(self) -> ResultValue {
        let [(indexable_nid, indexable_value), (index_nid, index_value)] = &self.arg_values[..]
        else {
            return self.incorrect_args(2..=2);
        };
        match indexable_value {
            Value::List(list) => {
                let Value::Integer(big_int) = index_value else {
                    return Err(self.cx.nid_err(
                        *index_nid,
                        RunnerError::ExpectedType {
                            actual: index_value.rain_type_id(),
                            expected: &[RainTypeId::Integer],
                        },
                    ));
                };
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
                let Value::String(s) = index_value else {
                    return Err(self.cx.nid_err(
                        *index_nid,
                        RunnerError::ExpectedType {
                            actual: index_value.rain_type_id(),
                            expected: &[RainTypeId::String],
                        },
                    ));
                };
                record.0.get(s.as_str()).cloned().ok_or_else(|| {
                    self.cx.nid_err(
                        self.nid,
                        RunnerError::IndexKeyNotFound(s.as_ref().to_owned()),
                    )
                })
            }
            _ => Err(self.cx.nid_err(
                *indexable_nid,
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
        record.insert(
            String::from("triple"),
            Value::String(Arc::new(String::from(env!("TARGET_PLATFORM")))), // Set by build script
        );
        Ok(Value::Record(Arc::new(RainRecord(record))))
    }

    fn string_contains(self) -> ResultValue {
        match &self.arg_values[..] {
            [(haystack_nid, haystack_value), (needle_nid, needle_value)] => {
                let Value::String(haystack) = haystack_value else {
                    return Err(self.cx.nid_err(
                        *haystack_nid,
                        RunnerError::ExpectedType {
                            actual: haystack_value.rain_type_id(),
                            expected: &[RainTypeId::String],
                        },
                    ));
                };
                let Value::String(needle) = needle_value else {
                    return Err(self.cx.nid_err(
                        *needle_nid,
                        RunnerError::ExpectedType {
                            actual: needle_value.rain_type_id(),
                            expected: &[RainTypeId::String],
                        },
                    ));
                };
                Ok(Value::Boolean(haystack.contains(&**needle)))
            }
            _ => self.incorrect_args(2..=2),
        }
    }

    fn export_to_local(self) -> ResultValue {
        match &self.arg_values[..] {
            [(src_nid, src_value), (dst_nid, dst_value)] => {
                let Value::File(src) = src_value else {
                    return Err(self.cx.nid_err(
                        *src_nid,
                        RunnerError::ExpectedType {
                            actual: src_value.rain_type_id(),
                            expected: &[RainTypeId::File],
                        },
                    ));
                };
                let Value::Dir(dst) = dst_value else {
                    return Err(self.cx.nid_err(
                        *dst_nid,
                        RunnerError::ExpectedType {
                            actual: dst_value.rain_type_id(),
                            expected: &[RainTypeId::Dir],
                        },
                    ));
                };
                match dst.area() {
                    FileArea::Local(_) => (),
                    _ => {
                        return Err(self.cx.nid_err(
                            *dst_nid,
                            RunnerError::Makeshift("destination must be in a local area".into()),
                        ));
                    }
                }
                let src = self.runner.driver.resolve_fs_entry(src.inner());
                let dst = self.runner.driver.resolve_fs_entry(dst.inner());
                let filename = src.file_name().ok_or_else(|| {
                    self.cx.nid_err(
                        self.nid,
                        RunnerError::Makeshift("src path does not have filename".into()),
                    )
                })?;
                let dst = dst.join(filename);
                // TODO: Move this to driver trait
                if let Err(err) = std::fs::copy(src, dst) {
                    return Err(self.cx.nid_err(self.nid, RunnerError::AreaIOError(err)));
                }
                Ok(Value::Unit)
            }
            _ => self.incorrect_args(2..=2),
        }
    }

    fn file_metadata(self) -> ResultValue {
        match &self.arg_values[..] {
            [(file_nid, file_value)] => {
                let Value::File(f) = file_value else {
                    return Err(self.cx.nid_err(
                        *file_nid,
                        RunnerError::ExpectedType {
                            actual: file_value.rain_type_id(),
                            expected: &[RainTypeId::File],
                        },
                    ));
                };
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
            _ => self.incorrect_args(1..=1),
        }
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
            [_, _] => {
                todo!("implement globbing")
            }
            _ => self.incorrect_args(1..=2),
        }
    }

    fn foreach(self) -> ResultValue {
        match &self.arg_values[..] {
            [(list_nid, list_value), (func_nid, func_value)] => {
                let Value::List(list) = list_value else {
                    return Err(self.cx.nid_err(
                        *list_nid,
                        RunnerError::ExpectedType {
                            actual: list_value.rain_type_id(),
                            expected: &[RainTypeId::List],
                        },
                    ));
                };
                let Value::Function(func) = func_value else {
                    return Err(self.cx.nid_err(
                        *func_nid,
                        RunnerError::ExpectedType {
                            actual: func_value.rain_type_id(),
                            expected: &[RainTypeId::Function],
                        },
                    ));
                };
                for item in list.0.clone() {
                    if self.cx.call_depth >= super::MAX_CALL_DEPTH {
                        return Err(self
                            .cx
                            .err(self.fn_call.lparen_token, RunnerError::MaxCallDepth));
                    }
                    let arg_values: Vec<Value> = vec![item];
                    let key = super::cache::CacheKey::Declaration {
                        declaration: *func,
                        args: arg_values.clone(),
                    };

                    if let Some(cache_entry) = self.runner.cache.get(&key) {
                        self.cx.deps.extend(cache_entry.deps);
                        return Ok(cache_entry.value);
                    }
                    let start = Instant::now();
                    let m = &Arc::clone(self.runner.ir.get_module(func.module_id()));
                    let nid = m.get_declaration(func.local_id());
                    let node = m.get(nid);
                    let Node::FnDeclare(fn_declare) = node else {
                        unreachable!();
                    };
                    let function_name = fn_declare.name.span.contents(&m.src);
                    self.runner.driver.enter_call(function_name);
                    if fn_declare.args.len() != 1 {
                        return Err(self.cx.err(
                            self.fn_call.rparen_token,
                            RunnerError::IncorrectArgs {
                                required: fn_declare.args.len()..=fn_declare.args.len(),
                                actual: 1,
                            },
                        ));
                    }
                    let args = fn_declare
                        .args
                        .iter()
                        .zip(arg_values)
                        .map(|(a, v)| (a.name.span.contents(&m.src), v))
                        .collect();
                    let mut callee_cx = Cx {
                        module: m,
                        call_depth: self.cx.call_depth + 1,
                        args,
                        locals: HashMap::new(),
                        deps: Vec::new(),
                    };
                    let result = self
                        .runner
                        .evaluate_node(&mut callee_cx, fn_declare.block)?;
                    self.runner.driver.exit_call(function_name);
                    self.runner.cache.put(
                        key,
                        CacheEntry {
                            execution_time: start.elapsed(),
                            expires: None,
                            etag: None,
                            deps: callee_cx.deps.clone(),
                            value: result.clone(),
                        },
                    );
                    self.cx.deps.extend(callee_cx.deps);
                }
                Ok(Value::Unit)
            }
            _ => self.incorrect_args(2..=2),
        }
    }
}
