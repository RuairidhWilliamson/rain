pub use ipc_impl::{Client, Connection, Incoming, Listener};

#[cfg(target_family = "unix")]
mod ipc_impl {
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::Path;

    pub struct Listener(UnixListener);

    impl Listener {
        pub fn bind(path: impl AsRef<Path>) -> std::io::Result<Self> {
            UnixListener::bind(path).map(Self)
        }

        pub fn incoming(&self) -> Incoming<'_> {
            Incoming(self.0.incoming())
        }
    }

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
}

#[cfg(target_family = "windows")]
mod impl_windows {
    compile_error! {"ipc not implemented on windows"}
}
