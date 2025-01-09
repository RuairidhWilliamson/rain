use std::{os::unix::net::UnixStream, process::Stdio, time::Duration};

use serde::de::DeserializeOwned;

use crate::config::Config;

use super::msg::{Request, RequestHeader, RequestTrait, ResponseWrapper, RestartReason};

#[derive(Debug)]
pub enum Error {
    CurrentExe,
    RestartLoop(RestartReason),
    TimeoutWaitingForServer,
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

pub fn make_request_or_start<Req>(config: &Config, request: Req) -> Result<Req::Response, Error>
where
    Req: RequestTrait,
{
    log::info!("Connecting");
    let stream = match UnixStream::connect(config.server_socket_path()) {
        Ok(s) => s,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            log::info!("No socket at path");
            start_server(config)?
        }
        Err(err) if err.kind() == std::io::ErrorKind::ConnectionRefused => {
            log::info!("Found stale socket, removing...");
            std::fs::remove_file(config.server_socket_path())?;
            start_server(config)?
        }
        Err(err) => {
            return Err(err.into());
        }
    };
    let exe_stat = crate::exe::current_exe_metadata().ok_or(Error::CurrentExe)?;
    let hdr = RequestHeader {
        config: config.clone(),
        modified_time: exe_stat.modified()?,
    };
    let request: Request = request.into();
    let response: ResponseWrapper<Req::Response> = make_request(stream, &hdr, &request)?;
    match response {
        ResponseWrapper::RestartPls(reason) => {
            log::info!("server requested restart, reason {reason:?}");
            let stream = start_server(config)?;
            match make_request(stream, &hdr, &request)? {
                ResponseWrapper::Response(resp) => Ok(resp),
                ResponseWrapper::RestartPls(reason) => Err(Error::RestartLoop(reason)),
            }
        }
        ResponseWrapper::Response(resp) => Ok(resp),
    }
}

fn start_server(config: &Config) -> Result<UnixStream, Error> {
    std::fs::create_dir_all(&config.base_cache_dir)?;
    log::info!("Starting server...");
    let p = std::process::Command::new(crate::exe::current_exe().ok_or(Error::CurrentExe)?)
        .env("RAIN_SERVER", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(config.server_stderr_path())?)
        .spawn()?;
    log::info!("Started {}", p.id());
    // Wait for the socket to be created
    for _ in 0..10 {
        match UnixStream::connect(config.server_socket_path()) {
            Ok(stream) => return Ok(stream),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            Err(err) => {
                return Err(err.into());
            }
        }
    }
    Err(Error::TimeoutWaitingForServer)
}

fn make_request<Resp>(
    mut stream: UnixStream,
    hdr: &RequestHeader,
    request: &Request,
) -> Result<ResponseWrapper<Resp>, Error>
where
    Resp: std::fmt::Debug + DeserializeOwned,
{
    ciborium::into_writer(hdr, &mut stream)?;
    ciborium::into_writer(request, &mut stream)?;
    let response: ResponseWrapper<Resp> = ciborium::from_reader(&mut stream)?;
    log::info!("Got repsonse {response:?}");
    Ok(response)
}
