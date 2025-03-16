#![allow(
    unsafe_code,
    clippy::undocumented_unsafe_blocks,
    clippy::missing_panics_doc
)]

use std::{ffi::CString, mem::MaybeUninit, path::Path};

use windows::{
    Win32::{
        Foundation::{GENERIC_READ, GENERIC_WRITE, HANDLE},
        Storage::FileSystem::{
            CreateFileA, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_MODE, FlushFileBuffers, OPEN_EXISTING,
            PIPE_ACCESS_DUPLEX, ReadFile, WriteFile,
        },
        System::Pipes::{ConnectNamedPipe, CreateNamedPipeA, PIPE_TYPE_BYTE},
    },
    core::{Owned, PCSTR},
};

#[derive(Debug)]
pub struct Listener {
    listener: Option<Owned<HANDLE>>,
    path: CString,
}

unsafe impl Send for Listener {}
unsafe impl Sync for Listener {}

impl Listener {
    pub fn bind(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let path = path.as_ref().to_str().expect("path is utf-8");
        assert!(
            path.starts_with("\\\\.\\pipe\\"),
            "path must start with unc pipe"
        );
        let path: CString = CString::new(path).expect("null char not in path");
        let mut l = Self {
            listener: None,
            path,
        };
        l.create_named_pipe()?;
        Ok(l)
    }

    fn create_named_pipe(&mut self) -> std::io::Result<()> {
        let handle = unsafe {
            Owned::new(CreateNamedPipeA(
                PCSTR(self.path.as_ptr().cast::<u8>()),
                PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_BYTE,
                4,
                0,
                0,
                0,
                None,
            )?)
        };
        self.listener = Some(handle);
        Ok(())
    }

    pub fn incoming(&mut self) -> Incoming<'_> {
        Incoming(self)
    }
}

#[derive(Debug)]
pub struct Incoming<'a>(&'a mut Listener);

impl Iterator for Incoming<'_> {
    type Item = Result<Connection, std::io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let handle = self.0.listener.take()?;
        if let Err(err) = self.0.create_named_pipe() {
            self.0.listener = Some(handle);
            return Some(Err(err));
        }
        if let Err(err) = unsafe { ConnectNamedPipe(*handle, None) } {
            self.0.listener = Some(handle);
            return Some(Err(err.into()));
        };
        Some(Ok(Connection(handle)))
    }
}

#[derive(Debug)]
pub struct Connection(Owned<HANDLE>);

unsafe impl Send for Connection {}
unsafe impl Sync for Connection {}

impl std::io::Read for Connection {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut read = MaybeUninit::uninit();
        unsafe { ReadFile(*self.0, Some(buf), Some(read.as_mut_ptr()), None)? };
        let read = unsafe { read.assume_init() };
        Ok(read as usize)
    }
}

impl std::io::Write for Connection {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut written = MaybeUninit::uninit();
        unsafe { WriteFile(*self.0, Some(buf), Some(written.as_mut_ptr()), None)? };
        let written = unsafe { written.assume_init() };
        Ok(written as usize)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        unsafe { FlushFileBuffers(*self.0) }?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct Client(Owned<HANDLE>);

unsafe impl Send for Client {}
unsafe impl Sync for Client {}

impl Client {
    pub fn connect(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let path = path_conversion(path);
        let handle = unsafe {
            Owned::new(CreateFileA(
                PCSTR(path.as_ptr().cast::<u8>()),
                GENERIC_READ.0 | GENERIC_WRITE.0,
                FILE_SHARE_MODE::default(),
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )?)
        };
        Ok(Self(handle))
    }
}

impl std::io::Read for Client {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut read = MaybeUninit::uninit();
        unsafe { ReadFile(*self.0, Some(buf), Some(read.as_mut_ptr()), None)? };
        let read = unsafe { read.assume_init() };
        Ok(read as usize)
    }
}

impl std::io::Write for Client {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut written = MaybeUninit::uninit();
        unsafe { WriteFile(*self.0, Some(buf), Some(written.as_mut_ptr()), None)? };
        let written = unsafe { written.assume_init() };
        Ok(written as usize)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        unsafe { FlushFileBuffers(*self.0) }?;
        Ok(())
    }
}

fn path_conversion(path: impl AsRef<Path>) -> CString {
    let path = path.as_ref().to_str().expect("path is utf-8");
    assert!(
        path.starts_with("\\\\.\\pipe\\"),
        "path must start with unc pipe"
    );
    let path: CString = CString::new(path).expect("null char not in path");
    path
}
