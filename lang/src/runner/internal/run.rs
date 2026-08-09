#![allow(clippy::unnecessary_wraps, clippy::needless_pass_by_value)]

use std::{borrow::Cow, collections::HashMap, sync::Arc};

use indexmap::IndexMap;

use crate::{
    afs::area::FileAreaRef,
    driver::{DriverTrait, RunOptions, monitoring::Call},
    runner::{
        cache::CacheTrait,
        dep::Dep,
        internal::{InternalCx, macros::expect_type},
    },
};

use crate::runner::{
    Result,
    error::RunnerError,
    value::{RainInteger, RainRecord, RainTypeId, Value},
};

impl<Driver: DriverTrait, Cache: CacheTrait> InternalCx<'_, '_, '_, Driver, Cache> {
    pub fn run(mut self) -> Result<Value> {
        self.add_deps_from_args();
        match &self.arg_values[..] {
            [
                (area_nid, area_value),
                (file_nid, file_value),
                (args_nid, args_value),
                (env_nid, env_value),
            ] => {
                let overlay_area: Option<FileAreaRef> = match area_value {
                    Value::Unit => None,
                    Value::GeneratedFSArea(area) => Some(area.as_ref().into()),
                    Value::LocalFSArea(area) => Some(area.as_ref().into()),
                    _ => Err(self.caller_cx.nid_err(
                        *area_nid,
                        RunnerError::ExpectedType {
                            actual: area_value.rain_type_id(),
                            expected: Cow::Borrowed(&[
                                RainTypeId::GeneratedFSArea,
                                RainTypeId::LocalFSArea,
                                RainTypeId::Unit,
                            ]),
                        },
                    ))?,
                };
                let bin = self.expect_file_path((*file_nid, file_value))?;
                let args = expect_type!(self, List, (args_nid, args_value));
                let args = args
                    .0
                    .iter()
                    .map(|value| {
                        self.runner
                            .stringify_value(self.caller_cx, *args_nid, value)
                    })
                    .collect::<Result<Vec<String>>>()?;
                let Value::Record(env) = env_value else {
                    return Err(self.caller_cx.nid_err(
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
                    .map(|(key, value)| {
                        Ok((
                            key.to_owned(),
                            self.runner
                                .stringify_value(self.caller_cx, *env_nid, value)?,
                        ))
                    })
                    .collect::<Result<HashMap<String, String>>>()?;

                let display_args = args.join(" ");
                let _call = self.runner.driver.call_guard(Call::Custom(format!(
                    "Run {} {display_args}",
                    bin.display()
                )));
                let status = self
                    .runner
                    .driver
                    .run(
                        overlay_area,
                        RunOptions {
                            bin: &bin,
                            args,
                            inherit_env: false,
                            env,
                            cancel: &self.runner.cancel,
                        },
                    )
                    .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
                let mut m = IndexMap::new();
                m.insert(
                    "success".to_owned(),
                    Value::Boolean(status.exit_code == Some(0)),
                );
                m.insert(
                    "exit_code".to_owned(),
                    Value::Integer(Arc::new(RainInteger(status.exit_code.unwrap_or(-1).into()))),
                );
                m.insert("area".to_owned(), status.area.to_value());
                m.insert(
                    "stdout".to_owned(),
                    Value::GeneratedFile(Arc::new(status.stdout)),
                );
                m.insert(
                    "stderr".to_owned(),
                    Value::GeneratedFile(Arc::new(status.stderr)),
                );
                Ok(Value::Record(Arc::new(RainRecord(m))))
            }
            _ => self.incorrect_args(4..=4),
        }
    }

    pub fn escape_run(mut self) -> Result<Value> {
        self.add_deps_from_args();
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
                let args = expect_type!(self, List, (args_nid, args_value));
                let args = args
                    .0
                    .iter()
                    .map(|value| {
                        self.runner
                            .stringify_value(self.caller_cx, *args_nid, value)
                    })
                    .collect::<Result<Vec<String>>>()?;
                let Value::Record(env) = env_value else {
                    return Err(self.caller_cx.nid_err(
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
                    .map(|(key, value)| {
                        Ok((
                            key.to_owned(),
                            self.runner
                                .stringify_value(self.caller_cx, *env_nid, value)?,
                        ))
                    })
                    .collect::<Result<HashMap<String, String>>>()?;
                let display_args = args.join(" ");
                let _call = self.runner.driver.call_guard(Call::Custom(format!(
                    "Run {} {display_args}",
                    bin.display()
                )));
                let status = self
                    .runner
                    .driver
                    .escape_run(
                        &dir,
                        RunOptions {
                            bin: &bin,
                            args,
                            inherit_env: true,
                            env,
                            cancel: &self.runner.cancel,
                        },
                    )
                    .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
                let mut m = IndexMap::new();
                m.insert(
                    "success".to_owned(),
                    Value::Boolean(status.exit_code == Some(0)),
                );
                m.insert(
                    "exit_code".to_owned(),
                    Value::Integer(Arc::new(RainInteger(status.exit_code.unwrap_or(-1).into()))),
                );
                m.insert(
                    "stdout".to_owned(),
                    Value::GeneratedFile(Arc::new(status.stdout)),
                );
                m.insert(
                    "stderr".to_owned(),
                    Value::GeneratedFile(Arc::new(status.stderr)),
                );
                Ok(Value::Record(Arc::new(RainRecord(m))))
            }
            _ => self.incorrect_args(4..=4),
        }
    }
}
