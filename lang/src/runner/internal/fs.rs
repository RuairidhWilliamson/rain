use std::{borrow::Cow, sync::Arc};

use indexmap::IndexMap;

use crate::{
    afs::{
        Dir, FSEntryTrait as _, File,
        absolute::AbsolutePathBuf,
        area::FileAreaRef,
        entry::{FSEntry, FSEntryRef},
        error::PathError,
        local::{entry::LocalFSEntry, file::LocalFile},
        path::SealedFilePath,
    },
    driver::{CreateAreaOptions, DriverTrait, FSEntryQueryResult, PathConflicts},
    runner::{
        Result,
        cache::CacheTrait,
        dep::Dep,
        error::RunnerError,
        internal::{
            InternalCx,
            macros::{expect_type, single_arg, three_args, two_args},
        },
        value::{RainInteger, RainList, RainRecord, RainTypeId, Value},
    },
};

impl<Driver: DriverTrait, Cache: CacheTrait> InternalCx<'_, '_, '_, Driver, Cache> {
    fn file_area_resolve_path(&mut self) -> Result<FSEntry> {
        match &self.arg_values[..] {
            [(relative_path_nid, relative_path_value)] => {
                let relative_path =
                    expect_type!(self, String, (*relative_path_nid, relative_path_value));
                let file = self
                    .caller_cx
                    .module
                    .file()
                    .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
                self.deps.push(Dep::CallingModule);
                let file_path = file
                    .path()
                    .parent()
                    .ok_or_else(|| {
                        self.caller_cx
                            .nid_err(self.nid, PathError::NoParentDirectory.into())
                    })?
                    .join(relative_path.as_str())
                    .map_err(|err| self.caller_cx.nid_err(*relative_path_nid, err.into()))?;
                Ok(FSEntry::new(file.area().to_owned_area(), file_path))
            }
            [(parent_nid, parent_value), (path_nid, path_value)] => {
                let path = expect_type!(self, String, (path_nid, path_value));
                match parent_value {
                    Value::GeneratedFSArea(area) => {
                        let file_path = SealedFilePath::new(path)
                            .map_err(|err| self.caller_cx.nid_err(*path_nid, err.into()))?;
                        Ok(FSEntry::new((*area).as_ref().clone().into(), file_path))
                    }
                    Value::LocalFSArea(area) => {
                        let file_path = SealedFilePath::new(path)
                            .map_err(|err| self.caller_cx.nid_err(*path_nid, err.into()))?;
                        Ok(FSEntry::new((*area).as_ref().clone().into(), file_path))
                    }
                    Value::GeneratedDir(dir) => {
                        let area = dir.area();
                        let base_path = dir.path();
                        let path = base_path
                            .join(path)
                            .map_err(|err| self.caller_cx.nid_err(*path_nid, err.into()))?;
                        Ok(FSEntry::new(area.to_owned_area(), path))
                    }
                    Value::LocalDir(dir) => {
                        let area = dir.area();
                        let base_path = dir.path();
                        let path = base_path
                            .join(path)
                            .map_err(|err| self.caller_cx.nid_err(*path_nid, err.into()))?;
                        Ok(FSEntry::new(area.to_owned_area(), path))
                    }
                    _ => Err(self.caller_cx.nid_err(
                        *parent_nid,
                        RunnerError::ExpectedType {
                            actual: parent_value.rain_type_id(),
                            expected: Cow::Borrowed(&[
                                RainTypeId::GeneratedFSArea,
                                RainTypeId::LocalFSArea,
                                RainTypeId::GeneratedDir,
                                RainTypeId::LocalDir,
                            ]),
                        },
                    )),
                }
            }
            _ => self.incorrect_args(1..=2),
        }
    }

    pub fn get_file(mut self) -> Result<Value> {
        let entry = self.file_area_resolve_path()?;
        match self
            .runner
            .driver
            .query_fs(entry.as_fs_entry_ref())
            .map_err(|err| {
                self.caller_cx
                    .nid_err(self.nid, RunnerError::AreaIOError(err))
            })? {
            FSEntryQueryResult::File => {
                let file = File::new_checked(self.runner.driver, entry).map_err(|err| {
                    self.caller_cx
                        .nid_err(self.nid, RunnerError::PathError(err))
                })?;
                Ok(file.to_value())
            }
            result => Err(self
                .caller_cx
                .nid_err(self.nid, RunnerError::FSQuery(Box::new(entry), result))),
        }
    }

    pub fn get_dir(mut self) -> Result<Value> {
        let entry = self.file_area_resolve_path()?;
        match self
            .runner
            .driver
            .query_fs(entry.as_fs_entry_ref())
            .map_err(|err| {
                self.caller_cx
                    .nid_err(self.nid, RunnerError::AreaIOError(err))
            })? {
            FSEntryQueryResult::Directory => {
                // Safety: Checked that the dir exists and is a dir
                let dir = unsafe { Dir::new(entry) };
                Ok(dir.to_value())
            }
            result => Err(self
                .caller_cx
                .nid_err(self.nid, RunnerError::FSQuery(Box::new(entry), result))),
        }
    }

    pub fn get_area(mut self) -> Result<Value> {
        self.add_deps_from_args();
        *self.cache_hint = false;
        let f = self.expect_file(single_arg!(self))?;
        Ok(f.area().to_owned_area().to_value())
    }

    pub fn sha256(mut self) -> Result<Value> {
        self.add_deps_from_args();
        let f = self.expect_file(single_arg!(self))?;

        let hash = match &f {
            File::Generated(..) => self
                .runner
                .driver
                .sha256(&f)
                .map_err(|err| self.caller_cx.nid_err(self.nid, err))?,
            // Local files already know their hash
            File::Local(local_file) => local_file.file_hash().0,
        };
        Ok(Value::String(Arc::new(base16::encode_lower(&hash))))
    }

    pub fn sha512(mut self) -> Result<Value> {
        self.add_deps_from_args();
        let f = self.expect_file(single_arg!(self))?;

        Ok(Value::String(Arc::new(base16::encode_lower(
            &self
                .runner
                .driver
                .sha512(&f)
                .map_err(|err| self.caller_cx.nid_err(self.nid, err))?,
        ))))
    }

    pub fn create_area(mut self) -> Result<Value> {
        self.add_deps_from_args();
        let ((dirs_nid, dirs_value), flatten_input_dirs, overwrite_conflicts) = three_args!(self);
        let dirs = expect_type!(self, List, (dirs_nid, dirs_value));
        let flatten_input_dirs = expect_type!(self, Boolean, flatten_input_dirs);
        let overwrite_conflicts = expect_type!(self, Boolean, overwrite_conflicts);
        let dirs: Vec<FSEntryRef> = dirs
            .0
            .iter()
            .map(|dir| match dir {
                Value::GeneratedDir(d) => Ok(d.fsinner().into()),
                Value::GeneratedFile(f) => Ok(f.fsinner().into()),
                Value::LocalFile(f) => Ok(f.fsinner().into()),
                Value::LocalDir(d) => Ok(d.fsinner().into()),
                _ => Err(self.caller_cx.nid_err(
                    dirs_nid,
                    RunnerError::ExpectedType {
                        actual: dir.rain_type_id(),
                        expected: Cow::Borrowed(&[
                            RainTypeId::GeneratedDir,
                            RainTypeId::GeneratedFile,
                            RainTypeId::LocalDir,
                            RainTypeId::LocalFile,
                        ]),
                    },
                )),
            })
            .collect::<Result<Vec<FSEntryRef>, _>>()?;
        for entry in &dirs {
            if entry.area().is_local() {
                self.deps.push(Dep::LocalDir);
            }
        }
        let merged_area = self
            .runner
            .driver
            .create_area(
                &dirs,
                &CreateAreaOptions {
                    flatten_input_dirs: *flatten_input_dirs,
                    conflicts: if *overwrite_conflicts {
                        PathConflicts::Overwrite
                    } else {
                        PathConflicts::Throw
                    },
                    ..Default::default()
                },
            )
            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
        Ok(merged_area.to_value())
    }

    pub fn create_write_area(mut self) -> Result<Value> {
        self.add_deps_from_args();
        // TODO: This probably could be cached but it has some weird behaviour
        self.deps.push(Dep::Uncacheable);
        let (dirs_nid, dirs_value) = single_arg!(self);
        let dirs = expect_type!(self, List, (dirs_nid, dirs_value));
        let dirs: Vec<FSEntryRef> = dirs
            .0
            .iter()
            .map(|dir| match dir {
                Value::GeneratedDir(d) => Ok(d.fsinner().into()),
                Value::GeneratedFile(f) => Ok(f.fsinner().into()),
                Value::LocalFile(f) => Ok(f.fsinner().into()),
                Value::LocalDir(d) => Ok(d.fsinner().into()),
                _ => Err(self.caller_cx.nid_err(
                    dirs_nid,
                    RunnerError::ExpectedType {
                        actual: dir.rain_type_id(),
                        expected: Cow::Borrowed(&[
                            RainTypeId::GeneratedDir,
                            RainTypeId::GeneratedFile,
                            RainTypeId::LocalDir,
                            RainTypeId::LocalFile,
                        ]),
                    },
                )),
            })
            .collect::<Result<Vec<FSEntryRef>, _>>()?;
        let merged_area = self
            .runner
            .driver
            .create_area(
                &dirs,
                &CreateAreaOptions {
                    flatten_input_dirs: true,
                    ..Default::default()
                },
            )
            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
        Ok(merged_area.to_value())
    }

    pub fn read_file(mut self) -> Result<Value> {
        self.add_deps_from_args();
        let f = self.expect_file(single_arg!(self))?;
        Ok(Value::String(Arc::new(
            self.runner.driver.read_file(&f).map_err(|err| {
                self.caller_cx
                    .nid_err(self.nid, RunnerError::AreaIOError(err))
            })?,
        )))
    }

    pub fn create_file(mut self) -> Result<Value> {
        self.add_deps_from_args();
        let (contents, name, executable) = three_args!(self);
        let contents = expect_type!(self, String, contents);
        let name = expect_type!(self, String, name);
        let executable = expect_type!(self, Boolean, executable);
        Ok(Value::GeneratedFile(Arc::new(
            self.runner
                .driver
                .create_file(contents.as_bytes(), name, *executable)
                .map_err(|err| self.caller_cx.nid_err(self.nid, err))?,
        )))
    }

    pub fn local_area(self) -> Result<Value> {
        let FileAreaRef::Local(current_area_path) = &self
            .caller_cx
            .module
            .file()
            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?
            .area()
        else {
            return Err(self
                .caller_cx
                .nid_err(self.nid, RunnerError::IllegalLocalArea));
        };
        // TODO: Determine if this dep is required or not
        // self.deps.push(Dep::LocalDir);
        let path = expect_type!(self, String, single_arg!(self));
        let area_path = current_area_path.join(path.as_ref());
        let area_path = AbsolutePathBuf::try_from(area_path.as_path()).map_err(|err| {
            self.caller_cx
                .nid_err(self.nid, RunnerError::AreaIOError(err))
        })?;
        let entry = LocalFSEntry::new(area_path, SealedFilePath::root());
        match self
            .runner
            .driver
            .query_fs((&entry).into())
            .map_err(|err| {
                self.caller_cx
                    .nid_err(self.nid, RunnerError::AreaIOError(err))
            })? {
            FSEntryQueryResult::Directory => Ok(entry.area().to_owned_area().to_value()),
            result => Err(self.caller_cx.nid_err(
                self.nid,
                RunnerError::FSQuery(Box::new(entry.into()), result),
            )),
        }
    }

    #[expect(clippy::too_many_lines)]
    pub fn export_to_local(mut self) -> Result<Value> {
        self.add_deps_from_args();
        self.deps.push(Dep::Uncacheable);
        match &self.arg_values[..] {
            [(src_nid, src_value), (dst_nid, dst_value)] => {
                let dst = self.expect_dir_or_area((*dst_nid, dst_value))?;
                match dst.area() {
                    FileAreaRef::Local(_) => (),
                    FileAreaRef::Generated(_) => {
                        return Err(self.caller_cx.nid_err(
                            *dst_nid,
                            RunnerError::Makeshift("destination must be in a local area".into()),
                        ));
                    }
                }
                match src_value {
                    Value::GeneratedFile(src) => {
                        let filename = src.path().last().ok_or_else(|| {
                            self.caller_cx.nid_err(
                                src_nid,
                                RunnerError::Makeshift("src path does not have filename".into()),
                            )
                        })?;
                        let dst_path = dst.path().join(filename).map_err(|err| {
                            self.caller_cx
                                .nid_err(self.nid, RunnerError::PathError(err))
                        })?;
                        let dst = FSEntry::new(dst.area().to_owned_area(), dst_path);

                        self.runner
                            .driver
                            .export_file(
                                &File::Generated(src.as_ref().clone()),
                                dst.as_fs_entry_ref(),
                            )
                            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
                        Ok(Value::Unit)
                    }
                    Value::GeneratedDir(src) => {
                        let filename = src.path().last().ok_or_else(|| {
                            self.caller_cx.nid_err(
                                src_nid,
                                RunnerError::Makeshift("src path does not have last part".into()),
                            )
                        })?;
                        let dst_path = dst.path().join(filename).map_err(|err| {
                            self.caller_cx
                                .nid_err(self.nid, RunnerError::PathError(err))
                        })?;
                        let dst = FSEntry::new(dst.area().to_owned_area(), dst_path);

                        self.runner
                            .driver
                            .export_dir(
                                &Dir::Generated(src.as_ref().clone()),
                                dst.as_fs_entry_ref(),
                            )
                            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
                        Ok(Value::Unit)
                    }
                    _ => Err(self.caller_cx.nid_err(
                        src_nid,
                        RunnerError::ExpectedType {
                            actual: src_value.rain_type_id(),
                            expected: Cow::Borrowed(&[
                                RainTypeId::GeneratedFile,
                                RainTypeId::GeneratedDir,
                            ]),
                        },
                    )),
                }
            }
            [
                (src_nid, src_value),
                (dst_nid, dst_value),
                (filename_nid, filename_value),
            ] => {
                let dst = self.expect_dir_or_area((*dst_nid, dst_value))?;
                let filename = expect_type!(self, String, (filename_nid, filename_value));
                match dst.area() {
                    FileAreaRef::Local(_) => (),
                    FileAreaRef::Generated(_) => {
                        return Err(self.caller_cx.nid_err(
                            *dst_nid,
                            RunnerError::Makeshift("destination must be in a local area".into()),
                        ));
                    }
                }

                let dst_path = dst.path().join(filename).map_err(|err| {
                    self.caller_cx
                        .nid_err(self.nid, RunnerError::PathError(err))
                })?;
                let dst = FSEntry::new(dst.area().to_owned_area(), dst_path);

                match src_value {
                    Value::GeneratedFile(src) => {
                        self.runner
                            .driver
                            .export_file(
                                &File::Generated(src.as_ref().clone()),
                                dst.as_fs_entry_ref(),
                            )
                            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
                        Ok(Value::Unit)
                    }
                    Value::GeneratedDir(src) => {
                        self.runner
                            .driver
                            .export_dir(
                                &Dir::Generated(src.as_ref().clone()),
                                dst.as_fs_entry_ref(),
                            )
                            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
                        Ok(Value::Unit)
                    }
                    Value::LocalFile(src) => {
                        self.runner
                            .driver
                            .export_file(&File::Local(src.as_ref().clone()), dst.as_fs_entry_ref())
                            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
                        Ok(Value::Unit)
                    }
                    Value::LocalDir(src) => {
                        self.runner
                            .driver
                            .export_dir(&Dir::Local(src.as_ref().clone()), dst.as_fs_entry_ref())
                            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
                        Ok(Value::Unit)
                    }
                    Value::LocalFSArea(area) => {
                        let src = Dir::root(area.as_ref().into());
                        self.runner
                            .driver
                            .export_dir(&src, dst.as_fs_entry_ref())
                            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
                        Ok(Value::Unit)
                    }
                    Value::GeneratedFSArea(area) => {
                        let src = Dir::root(area.as_ref().into());
                        self.runner
                            .driver
                            .export_dir(&src, dst.as_fs_entry_ref())
                            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
                        Ok(Value::Unit)
                    }
                    _ => Err(self.caller_cx.nid_err(
                        src_nid,
                        RunnerError::ExpectedType {
                            actual: src_value.rain_type_id(),
                            expected: Cow::Borrowed(&[
                                RainTypeId::GeneratedFile,
                                RainTypeId::GeneratedDir,
                                RainTypeId::GeneratedFSArea,
                                RainTypeId::LocalFSArea,
                            ]),
                        },
                    )),
                }
            }
            _ => self.incorrect_args(2..=3),
        }
    }

    #[expect(clippy::too_many_lines)]
    pub fn check_export_to_local(mut self) -> Result<Value> {
        self.add_deps_from_args();
        match &self.arg_values[..] {
            [(src_nid, src_value), (dst_nid, dst_value)] => {
                let src = expect_type!(self, GeneratedFile, (src_nid, src_value));
                let dst = expect_type!(self, LocalDir, (dst_nid, dst_value));
                let filename = src.path().last().ok_or_else(|| {
                    self.caller_cx.nid_err(
                        self.nid,
                        RunnerError::Makeshift("src path does not have filename".into()),
                    )
                })?;
                let dst_path = dst.path().join(filename).map_err(|err| {
                    self.caller_cx
                        .nid_err(self.nid, RunnerError::PathError(err))
                })?;
                let entry = FSEntry::new(dst.area().to_owned_area(), dst_path);
                match self
                    .runner
                    .driver
                    .query_fs(entry.as_fs_entry_ref())
                    .map_err(|err| {
                        self.caller_cx
                            .nid_err(self.nid, RunnerError::AreaIOError(err))
                    })? {
                    FSEntryQueryResult::File => {}
                    _ => {
                        return Err(self.caller_cx.nid_err(
                            self.nid,
                            RunnerError::Makeshift("exported file does not exist".into()),
                        ));
                    }
                }
                let dst = File::new_checked(self.runner.driver, entry).map_err(|err| {
                    self.caller_cx
                        .nid_err(self.nid, RunnerError::PathError(err))
                })?;
                let src_contents = self
                    .runner
                    .driver
                    .read_file(&File::Generated(src.as_ref().clone()))
                    .map_err(|err| {
                        self.caller_cx
                            .nid_err(self.nid, RunnerError::AreaIOError(err))
                    })?;
                let dst_contents = self.runner.driver.read_file(&dst).map_err(|err| {
                    self.caller_cx
                        .nid_err(self.nid, RunnerError::AreaIOError(err))
                })?;
                if src_contents != dst_contents {
                    return Err(self.caller_cx.nid_err(
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
                let src = expect_type!(self, GeneratedFile, (src_nid, src_value));
                let dst = expect_type!(self, LocalDir, (dst_nid, dst_value));
                let filename = expect_type!(self, String, (filename_nid, filename_value));
                let dst_path = dst.path().join(filename).map_err(|err| {
                    self.caller_cx
                        .nid_err(self.nid, RunnerError::PathError(err))
                })?;
                let entry = LocalFSEntry::new(dst.fsinner().area.clone(), dst_path);
                match self
                    .runner
                    .driver
                    .query_fs((&entry).into())
                    .map_err(|err| {
                        self.caller_cx
                            .nid_err(self.nid, RunnerError::AreaIOError(err))
                    })? {
                    FSEntryQueryResult::File => {}
                    _ => {
                        return Err(self.caller_cx.nid_err(
                            self.nid,
                            RunnerError::Makeshift("exported file does not exist".into()),
                        ));
                    }
                }
                let dst = LocalFile::new_checked(self.runner.driver, entry).map_err(|err| {
                    self.caller_cx
                        .nid_err(self.nid, RunnerError::PathError(err))
                })?;
                let src_contents = self
                    .runner
                    .driver
                    .read_file(&File::Generated(src.as_ref().clone()))
                    .map_err(|err| {
                        self.caller_cx
                            .nid_err(self.nid, RunnerError::AreaIOError(err))
                    })?;
                let dst_contents =
                    self.runner
                        .driver
                        .read_file(&File::Local(dst))
                        .map_err(|err| {
                            self.caller_cx
                                .nid_err(self.nid, RunnerError::AreaIOError(err))
                        })?;
                if src_contents != dst_contents {
                    return Err(self.caller_cx.nid_err(
                        self.nid,
                        RunnerError::Makeshift("exported file does not match".into()),
                    ));
                }

                Ok(Value::Unit)
            }
            _ => self.incorrect_args(2..=3),
        }
    }

    pub fn file_metadata(mut self) -> Result<Value> {
        self.add_deps_from_args();
        let f = self.expect_file(single_arg!(self))?;
        let metadata = self
            .runner
            .driver
            .file_metadata(&f)
            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
        let mut record = IndexMap::new();
        record.insert(
            "size".to_owned(),
            Value::Integer(Arc::new(RainInteger(metadata.size.into()))),
        );
        Ok(Value::Record(Arc::new(RainRecord(record))))
    }

    pub fn glob(mut self) -> Result<Value> {
        self.add_deps_from_args();
        match &self.arg_values[..] {
            [(dir_nid, dir_value)] => {
                let d = self.expect_dir_or_area((*dir_nid, dir_value))?;
                let files = self
                    .runner
                    .driver
                    .glob(&d, "**/*")
                    .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
                let files: Vec<Value> = files.into_iter().map(File::to_value).collect();
                Ok(Value::List(Arc::new(RainList(files))))
            }
            [(dir_nid, dir_value), (pattern_nid, pattern_value)] => {
                let d = self.expect_dir_or_area((*dir_nid, dir_value))?;
                let pattern = expect_type!(self, String, (pattern_nid, pattern_value));
                let files = self
                    .runner
                    .driver
                    .glob(&d, pattern)
                    .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
                let files: Vec<Value> = files.into_iter().map(File::to_value).collect();
                Ok(Value::List(Arc::new(RainList(files))))
            }
            _ => self.incorrect_args(1..=2),
        }
    }

    pub fn file_name(mut self) -> Result<Value> {
        self.add_deps_from_args();
        let file = single_arg!(self);
        let file = self.expect_file(file)?;
        let Some(name) = file.path().last() else {
            return Err(self.caller_cx.nid_err(
                self.nid,
                RunnerError::Makeshift("file doesn't have a name".into()),
            ));
        };
        Ok(Value::String(Arc::new(name.to_string())))
    }

    pub fn copy_dir(mut self) -> Result<Value> {
        self.add_deps_from_args();
        let (dir, name) = two_args!(self);
        let dir = self.expect_dir_or_area(dir)?;
        let name = expect_type!(self, String, name);
        let out = self
            .runner
            .driver
            .copy_dir(&dir, name, true)
            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
        Ok(Value::GeneratedDir(Arc::new(out)))
    }
}
