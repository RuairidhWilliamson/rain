use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;

#[derive(Debug)]
pub struct Listener(UnixListener);

impl Listener {
    #[expect(unsafe_code, clippy::undocumented_unsafe_blocks)]
    pub fn bind(path: impl AsRef<Path>) -> std::io::Result<Self> {
        // Set the socket file to have rwx------ so that only the owner can access it
        // FIXME: This can race other threads affecting their create files
        let prev = unsafe { libc::umask(0o077) };
        let res = UnixListener::bind(path).map(Self);
        unsafe { libc::umask(prev) };
        res
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
        UnixStream::connect(path).map(Self)
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
