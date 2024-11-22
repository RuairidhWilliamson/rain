use std::{
    os::unix::net::{UnixListener, UnixStream},
    time::SystemTime,
};

use crate::{config::Config, remote::msg::RestartReason};

use super::msg::{Request, RequestHeader, RequestTrait, ResponseWrapper};

#[derive(Debug)]
pub enum Error {
    CurrentExe,
    IO(std::io::Error),
    Encode(ciborium::ser::Error<std::io::Error>),
    Decode(ciborium::de::Error<std::io::Error>),
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::IO(err)
    }
}

impl From<ciborium::ser::Error<std::io::Error>> for Error {
    fn from(err: ciborium::ser::Error<std::io::Error>) -> Self {
        Self::Encode(err)
    }
}

impl From<ciborium::de::Error<std::io::Error>> for Error {
    fn from(err: ciborium::de::Error<std::io::Error>) -> Self {
        Self::Decode(err)
    }
}

pub fn rain_server(config: Config) -> Result<(), Error> {
    let exe_stat = crate::exe::current_exe_metadata().ok_or(Error::CurrentExe)?;
    let modified_time = exe_stat.modified()?;
    let s = Server {
        config,
        modified_time,
    };
    let l = UnixListener::bind(s.config.server_socket_path())?;
    for stream in l.incoming() {
        match stream {
            Ok(stream) => {
                log::info!("got a stream {stream:?}");
                ClientHandler { server: &s, stream }.handle_client()?;
            }
            Err(err) => {
                log::error!("unix listener error: {err}");
            }
        }
    }
    todo!()
}

struct Server {
    config: Config,
    modified_time: SystemTime,
}

struct ClientHandler<'a> {
    server: &'a Server,
    stream: UnixStream,
}

impl ClientHandler<'_> {
    fn handle_client(mut self) -> Result<(), Error> {
        let hdr: RequestHeader = ciborium::from_reader(&mut self.stream)?;
        if hdr.modified_time != self.server.modified_time {
            log::info!("Restarting because modified time does not match");
            std::fs::remove_file(self.server.config.server_socket_path())?;
            let response = ResponseWrapper::<()>::RestartPls(RestartReason::RainBinaryChanged);
            ciborium::into_writer(&response, &mut self.stream)?;
            std::process::exit(0)
        }
        let request: Request = ciborium::from_reader(&mut self.stream)?;
        log::info!("Header {hdr:?}");
        log::info!("Request {request:?}");
        self.handle_request(request)
    }

    fn handle_request(self, req: Request) -> Result<(), Error> {
        match req {
            Request::Info(req) => {
                let resp = super::msg::info::InfoResponse {
                    pid: std::process::id(),
                    config: self.server.config.clone(),
                };
                self.send_response(&req, resp)?;
                Ok(())
            }
            Request::Shutdown(req) => {
                log::info!("Goodbye");
                self.send_response(&req, super::msg::shutdown::Goodbye)?;
                std::process::exit(0);
            }
        }
    }

    fn send_response<Req>(
        mut self,
        _req: &Req,
        response: Req::Response,
    ) -> Result<(), ciborium::ser::Error<std::io::Error>>
    where
        Req: RequestTrait,
    {
        let wrapped = ResponseWrapper::Response(response);
        ciborium::into_writer(&wrapped, &mut self.stream)
    }
}
