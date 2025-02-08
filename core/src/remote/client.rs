use std::{process::Stdio, time::Duration};

use crate::{config::Config, remote::msg::RequestWrapper};

use super::msg::{Message, Request, RequestTrait, RestartReason};

const MAX_RESTARTS: usize = 1;

#[derive(Debug)]
pub enum Error {
    CurrentExe,
    RestartLoop(RestartReason),
    TimeoutWaitingForServer,
    IO(std::io::Error),
    Encode(ciborium::ser::Error<std::io::Error>),
    Decode(ciborium::de::Error<std::io::Error>),
    ServerPanic,
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

pub fn make_request_or_start<Req>(
    config: &Config,
    request: Req,
    handle: impl Fn(Req::Intermediate),
) -> Result<Req::Response, Error>
where
    Req: RequestTrait,
{
    log::info!("Connecting");
    let mut stream = match crate::ipc::Client::connect(config.server_socket_path()) {
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
    let request: Request = request.into();
    let mut buf = Vec::new();
    ciborium::into_writer(&request, &mut buf)?;
    let req = RequestWrapper {
        config: config.clone(),
        modified_time: exe_stat.modified()?,
        request: buf,
    };
    let mut restart_attempt = 0;
    loop {
        log::debug!("sending request {req:?}");
        ciborium::into_writer(&req, &mut stream)?;
        loop {
            let msg: Message = ciborium::from_reader(&mut stream)?;
            match msg {
                Message::Intermediate(im) => {
                    let im: <Req as RequestTrait>::Intermediate =
                        ciborium::from_reader(std::io::Cursor::new(im))?;
                    handle(im);
                    continue;
                }
                Message::ServerPanic => {
                    log::error!("server panic");
                    return Err(Error::ServerPanic);
                }
                Message::RestartPls(reason) => {
                    if restart_attempt > MAX_RESTARTS {
                        return Err(Error::RestartLoop(reason));
                    }
                    restart_attempt += 1;
                    log::info!("server requested restart, reason {reason:?}");
                    stream = start_server(config)?;
                    break;
                }
                Message::Response(response) => {
                    return Ok(ciborium::from_reader(std::io::Cursor::new(response))?)
                }
            }
        }
    }
}

fn start_server(config: &Config) -> Result<crate::ipc::Client, Error> {
    std::fs::create_dir_all(&config.base_cache_dir)?;
    log::info!("Starting server...");
    let p = std::process::Command::new(crate::exe::current_exe().ok_or(Error::CurrentExe)?)
        .env("RAIN_SERVER", "1")
        .env("RAIN_LOG", "debug")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(config.server_stderr_path())?)
        .spawn()?;
    log::info!("Started {}", p.id());
    // Wait for the socket to be created
    for _ in 0..10 {
        match crate::ipc::Client::connect(config.server_socket_path()) {
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
