use std::sync::Arc;

use tracing::error;

use crate::{
    driver::DriverTrait,
    runner::{
        ResultValue,
        cache::CacheTrait,
        error::RunnerError,
        internal::{
            InternalCx,
            macros::{expect_type, single_arg, three_args, two_args},
        },
        value::Value,
    },
};

impl<Driver: DriverTrait, Cache: CacheTrait> InternalCx<'_, '_, '_, Driver, Cache> {
    pub fn extract_zip(mut self) -> ResultValue {
        self.add_deps_from_args();
        let f = self.expect_file(single_arg!(self))?;
        let area = self
            .runner
            .driver
            .extract_zip(&f)
            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
        Ok(area.to_value())
    }

    pub fn extract_gzip(mut self) -> ResultValue {
        self.add_deps_from_args();
        let (file, name) = two_args!(self);
        let file = self.expect_file(file)?;
        let name = expect_type!(self, String, name);
        let area = self
            .runner
            .driver
            .extract_gzip(&file, name)
            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
        Ok(Value::GeneratedFile(Arc::new(area)))
    }

    pub fn extract_xz(mut self) -> ResultValue {
        self.add_deps_from_args();
        let (file, name) = two_args!(self);
        let file = self.expect_file(file)?;
        let name = expect_type!(self, String, name);
        let area = self
            .runner
            .driver
            .extract_xz(&file, name)
            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
        Ok(Value::GeneratedFile(Arc::new(area)))
    }

    pub fn extract_tar(mut self) -> ResultValue {
        self.add_deps_from_args();
        let f = self.expect_file(single_arg!(self))?;
        let area = self
            .runner
            .driver
            .extract_tar(&f)
            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
        Ok(area.to_value())
    }

    pub fn compress_zstd(mut self) -> ResultValue {
        self.add_deps_from_args();
        let (file, name, level) = three_args!(self);
        let file = self.expect_file(file)?;
        let name = expect_type!(self, String, name);
        let level = expect_type!(self, Integer, level);

        let level: u8 = (&level.0).try_into().map_err(|err| {
            error!("compress zstd invalid level: {err}");
            self.caller_cx.nid_err(
                self.nid,
                RunnerError::Makeshift("level must be in the range 0 - 22".into()),
            )
        })?;
        if level > 22 {
            return Err(self.caller_cx.nid_err(
                self.nid,
                RunnerError::Makeshift("level must be in the range 0 - 22".into()),
            ));
        }
        Ok(Value::GeneratedFile(Arc::new(
            self.runner
                .driver
                .compress_zstd(&file, name, level)
                .map_err(|err| self.caller_cx.nid_err(self.nid, err))?,
        )))
    }

    pub fn extract_zstd(mut self) -> ResultValue {
        self.add_deps_from_args();
        let (file, name) = two_args!(self);
        let file = self.expect_file(file)?;
        let name = expect_type!(self, String, name);
        let area = self
            .runner
            .driver
            .extract_zstd(&file, name)
            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
        Ok(Value::GeneratedFile(Arc::new(area)))
    }

    pub fn create_tar(mut self) -> ResultValue {
        self.add_deps_from_args();
        let (dir, name) = two_args!(self);
        let dir = self.expect_dir_or_area(dir)?;
        let name = expect_type!(self, String, name);
        Ok(Value::GeneratedFile(Arc::new(
            self.runner
                .driver
                .create_tar(&dir, name)
                .map_err(|err| self.caller_cx.nid_err(self.nid, err))?,
        )))
    }

    pub fn compress_gzip(mut self) -> ResultValue {
        self.add_deps_from_args();
        let (file, name) = two_args!(self);
        let file = self.expect_file(file)?;
        let name = expect_type!(self, String, name);
        Ok(Value::GeneratedFile(Arc::new(
            self.runner
                .driver
                .compress_gzip(&file, name)
                .map_err(|err| self.caller_cx.nid_err(self.nid, err))?,
        )))
    }
}
