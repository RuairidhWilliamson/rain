#![allow(clippy::unnecessary_wraps, clippy::needless_pass_by_value)]

use std::{borrow::Cow, collections::HashMap, sync::Arc};

use indexmap::IndexMap;

use crate::{
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
                let bin = self.expect_file_path((*file_nid, file_value))?;
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
                    .map(|value| self.stringify_arg(*args_nid, value))
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
                        &bin,
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
                m.insert("area".to_owned(), status.area.to_value());
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
                let dir = self.expect_dir_or_area((*area_nid, area_value))?;
                let bin = self.expect_file_path((*file_nid, file_value))?;
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
                    .map(|value| self.stringify_arg(*args_nid, value))
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
                        &bin,
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
        Ok((key.to_owned(), self.stringify_impl(env_nid, value)?))
    }

    fn stringify_arg(&self, nid: NodeId, value: &Value) -> Result<String> {
        self.stringify_impl(nid, value)
    }
}
