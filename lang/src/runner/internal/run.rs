#![allow(clippy::unnecessary_wraps, clippy::needless_pass_by_value)]

use std::{borrow::Cow, collections::HashMap, sync::Arc};

use indexmap::IndexMap;

use crate::{
    afs::{dir::Dir, entry::FSEntryTrait as _},
    ast::NodeId,
    driver::{DriverTrait, RunOptions},
    runner::{cache::CacheTrait, dep::Dep},
};

use crate::runner::{
    Result, ResultValue,
    error::RunnerError,
    value::{RainInteger, RainRecord, RainTypeId, Value},
};

use super::{InternalCx, enter_call};

impl<Driver: DriverTrait, Cache: CacheTrait> InternalCx<'_, '_, '_, Driver, Cache> {
    pub fn run(self) -> ResultValue {
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
                            expected: Cow::Borrowed(&[RainTypeId::FileArea, RainTypeId::Unit]),
                        },
                    ))?,
                };
                let bin = match file_value {
                    Value::File(file) => &self.runner.driver.resolve_fs_entry(file.inner()),
                    Value::EscapeFile(escaped_file) => escaped_file.0.as_path(),
                    _ => {
                        return Err(self.cx.nid_err(
                            *file_nid,
                            RunnerError::ExpectedType {
                                actual: file_value.rain_type_id(),
                                expected: Cow::Borrowed(&[
                                    RainTypeId::File,
                                    RainTypeId::EscapeFile,
                                ]),
                            },
                        ));
                    }
                };
                let Value::List(args) = args_value else {
                    return Err(self.cx.nid_err(
                        *args_nid,
                        RunnerError::ExpectedType {
                            actual: args_value.rain_type_id(),
                            expected: Cow::Borrowed(&[RainTypeId::List]),
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
                            expected: Cow::Borrowed(&[RainTypeId::List]),
                        },
                    ));
                };
                let env = env
                    .0
                    .iter()
                    .map(|(key, value)| self.stringify_env(*env_nid, key, value))
                    .collect::<Result<HashMap<String, String>>>()?;

                let display_args = args.join(" ");
                let _call = enter_call(
                    self.runner.driver,
                    format!("Run {} {display_args}", bin.display()),
                );
                let status = self
                    .runner
                    .driver
                    .run(
                        overlay_area,
                        bin,
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

    pub fn escape_run(self) -> ResultValue {
        self.deps.push(Dep::Escape);
        self.check_escape_mode()?;
        match &self.arg_values[..] {
            [
                (area_nid, area_value),
                (file_nid, file_value),
                (args_nid, args_value),
                (env_nid, env_value),
            ] => {
                let dir = self.expect_dir_or_area(*area_nid, area_value)?;
                let bin = match file_value {
                    Value::File(file) => &self.runner.driver.resolve_fs_entry(file.inner()),
                    Value::EscapeFile(escaped_file) => escaped_file.0.as_path(),
                    _ => {
                        return Err(self.cx.nid_err(
                            *file_nid,
                            RunnerError::ExpectedType {
                                actual: file_value.rain_type_id(),
                                expected: Cow::Borrowed(&[
                                    RainTypeId::File,
                                    RainTypeId::EscapeFile,
                                ]),
                            },
                        ));
                    }
                };
                let Value::List(args) = args_value else {
                    return Err(self.cx.nid_err(
                        *args_nid,
                        RunnerError::ExpectedType {
                            actual: args_value.rain_type_id(),
                            expected: Cow::Borrowed(&[RainTypeId::List]),
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
                            expected: Cow::Borrowed(&[RainTypeId::List]),
                        },
                    ));
                };
                let env = env
                    .0
                    .iter()
                    .map(|(key, value)| self.stringify_env(*env_nid, key, value))
                    .collect::<Result<HashMap<String, String>>>()?;
                let display_args = args.join(" ");
                let _call = enter_call(
                    self.runner.driver,
                    format!("Run {} {display_args}", bin.display()),
                );
                let status = self
                    .runner
                    .driver
                    .escape_run(
                        &dir,
                        bin,
                        args,
                        RunOptions {
                            inherit_env: true,
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
                    expected: Cow::Borrowed(&[
                        RainTypeId::String,
                        RainTypeId::File,
                        RainTypeId::Dir,
                        RainTypeId::FileArea,
                    ]),
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
                    expected: Cow::Borrowed(&[
                        RainTypeId::String,
                        RainTypeId::File,
                        RainTypeId::Dir,
                        RainTypeId::FileArea,
                    ]),
                },
            )),
        }
    }
}
