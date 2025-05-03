//! Ruipc is a simple IPC library providing an interface that allows inter process communication on Windows, macOS and Linux.
//! It does not support communication across OSes or across networks.

// #![warn(missing_docs)]

#[cfg(target_family = "unix")]
mod unix;
#[cfg(target_family = "windows")]
mod windows;

#[cfg(test)]
mod tests;

#[cfg(target_family = "unix")]
use unix as sys;
#[cfg(target_family = "windows")]
use windows as sys;

use std::{
    io::{Read, Result, Write},
    path::Path,
};

/// Listener is created after binding to a particular address and can recieve incoming connections from clients by calling [`incoming`].
#[derive(Debug)]
pub struct Listener(sys::Listener);

impl Listener {
    pub fn bind(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self(sys::Listener::bind(path)?))
    }

    pub fn incoming(&mut self) -> Incoming<'_> {
        Incoming(self.0.incoming())
    }
}

#[derive(Debug)]
pub struct Incoming<'a>(sys::Incoming<'a>);

impl Iterator for Incoming<'_> {
    type Item = Result<Connection>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.0.next()?.map(Connection))
    }
}

#[derive(Debug)]
pub struct Connection(sys::Connection);

impl Read for Connection {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.0.read(buf)
    }
}

impl Write for Connection {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        self.0.flush()
    }
}

#[derive(Debug)]
pub struct Client(sys::Client);

impl Client {
    pub fn connect(path: impl AsRef<Path>) -> std::io::Result<Self> {
        Ok(Self(sys::Client::connect(path)?))
    }
}

impl Read for Client {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.0.read(buf)
    }
}

impl Write for Client {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        self.0.flush()
    }
}
