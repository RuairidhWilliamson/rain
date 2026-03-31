use std::fs::DirBuilder;
use std::os::unix::fs::DirBuilderExt as _;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;

#[derive(Debug)]
pub struct Listener(UnixListener);

impl Listener {
    pub fn bind(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let path = path.as_ref();
        // Set the directory of the socket file to have rwx------ so that only the owner can access it
        DirBuilder::new().mode(0o700).recursive(true).create(path)?;

        UnixListener::bind(path.join("socket")).map(Self)
    }

    pub fn incoming(&self) -> Incoming<'_> {
        Incoming(self.0.incoming())
    }
}

#[derive(Debug)]
pub struct Incoming<'a>(std::os::unix::net::Incoming<'a>);

impl Iterator for Incoming<'_> {
    type Item = Result<Connection, std::io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|res| res.map(Connection))
    }
}

#[derive(Debug)]
pub struct Connection(UnixStream);

impl std::io::Read for Connection {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl std::io::Write for Connection {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

#[derive(Debug)]
pub struct Client(UnixStream);

impl Client {
    pub fn connect(path: impl AsRef<Path>) -> std::io::Result<Self> {
        UnixStream::connect(path.as_ref().join("socket")).map(Self)
    }
}

impl std::io::Read for Client {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl std::io::Write for Client {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}
