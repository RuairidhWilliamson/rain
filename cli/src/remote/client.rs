#![allow(clippy::unwrap_used, clippy::too_many_lines)]

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use rain_core::config::Config;

use crate::remote::{
    msg::{RequestHeader, RequestWrapper},
    server::InternalMsgConnection,
};

use super::msg::{Request, RequestTrait, RestartReason, ServerMessage};

const MAX_RESTARTS: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientMode {
    BackgroundThread,
    #[expect(dead_code)]
    ForkProcess,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not fetch current executable")]
    CurrentExe,
    #[error("detected server restart loop due to {0:?}")]
    RestartLoop(RestartReason),
    #[error("timeout waiting for server to start")]
    TimeoutWaitingForServer,
    #[error("io error: {0}")]
    IO(std::io::Error),
    #[error("encode error: {0}")]
    Encode(ciborium::ser::Error<std::io::Error>),
    #[error("decode error: {0}")]
    Decode(ciborium::de::Error<std::io::Error>),
    #[error("server panic see log: {0}")]
    ServerPanic(PathBuf),
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
    mut handle: impl FnMut(Req::Intermediate),
    client_mode: ClientMode,
) -> Result<Req::Response, Error>
where
    Req: RequestTrait,
{
    let exe = crate::exe::current_exe()
        .ok_or(Error::CurrentExe)?
        .to_path_buf();
    let exe_stat = crate::exe::current_exe_metadata().ok_or(Error::CurrentExe)?;
    match client_mode {
        ClientMode::ForkProcess => {
            log::info!("Connecting");
            let mut stream = match ruipc::Client::connect(config.server_socket_path()) {
                Ok(s) => s,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                    log::info!("No socket at path");
                    spawn_local_server(config)?
                }
                Err(err) if err.kind() == std::io::ErrorKind::ConnectionRefused => {
                    log::info!("Found stale socket, removing...");
                    std::fs::remove_file(config.server_socket_path())?;
                    spawn_local_server(config)?
                }
                Err(err) => {
                    return Err(err.into());
                }
            };
            let request: Request = request.into();
            let mut buf = Vec::new();
            ciborium::into_writer(&request, &mut buf)?;
            let req = RequestWrapper {
                header: RequestHeader {
                    config: config.clone(),
                    modified_time: exe_stat.modified()?,
                    exe,
                },
                request: buf,
            };
            let mut restart_attempt = 0;
            loop {
                log::debug!("sending request {req:?}");
                ciborium::into_writer(&req, &mut stream)?;
                loop {
                    let msg: ServerMessage = ciborium::from_reader(&mut stream)?;
                    match msg {
                        ServerMessage::Intermediate(im) => {
                            let im: <Req as RequestTrait>::Intermediate =
                                ciborium::from_reader(std::io::Cursor::new(im))?;
                            handle(im);
                        }
                        ServerMessage::ServerPanic => {
                            log::error!("server panic");
                            let panic_path = config.server_panic_path(uuid::Uuid::new_v4());
                            let _ =
                                std::fs::create_dir_all(panic_path.parent().expect("parent path"));
                            match std::fs::hard_link(config.server_stderr_path(), &panic_path) {
                                Err(err) => {
                                    log::error!("failed to hardlink panic: {err}");
                                    return Err(Error::ServerPanic(config.server_stderr_path()));
                                }
                                Ok(()) => return Err(Error::ServerPanic(panic_path)),
                            }
                        }
                        ServerMessage::RestartPls(reason) => {
                            if restart_attempt > MAX_RESTARTS {
                                return Err(Error::RestartLoop(reason));
                            }
                            restart_attempt += 1;
                            log::info!("server requested restart, reason {reason:?}");
                            stream = spawn_local_server(config)?;
                            break;
                        }
                        ServerMessage::Response(response) => {
                            return Ok(ciborium::from_reader(std::io::Cursor::new(response))?);
                        }
                    }
                }
            }
        }
        ClientMode::BackgroundThread => {
            let (stream, tx, rx) = InternalMsgConnection::new();
            let server_thread_handle = {
                let config = config.clone();
                std::thread::spawn(move || {
                    let server = super::server::Server::new(config).unwrap();
                    let client_handler = super::server::ClientHandler {
                        server: &server,
                        stream,
                    };
                    let result = client_handler.handle_client();
                    match result {
                        Ok(()) | Err(super::server::Error::GracefulExit) => (),
                        Err(err) => eprintln!("server error: {err:#}"),
                    }
                })
            };
            let mut buf = Vec::new();
            let request = request.into();
            ciborium::into_writer(&request, &mut buf)?;
            let req = RequestWrapper {
                header: RequestHeader {
                    config: config.clone(),
                    modified_time: exe_stat.modified()?,
                    exe,
                },
                request: buf,
            };
            tx.send(req).unwrap();
            loop {
                let msg = rx.recv().unwrap();
                match msg {
                    ServerMessage::ServerPanic => todo!(),
                    ServerMessage::RestartPls(_restart_reason) => todo!(),
                    ServerMessage::Intermediate(im) => {
                        let im: <Req as RequestTrait>::Intermediate =
                            ciborium::from_reader(std::io::Cursor::new(im))?;
                        handle(im);
                    }
                    ServerMessage::Response(response) => {
                        log::debug!("waiting for server to finish");
                        if let Err(err) = server_thread_handle.join() {
                            log::error!("server panicked waiting for finish {err:?}");
                        }
                        return Ok(ciborium::from_reader(std::io::Cursor::new(response))?);
                    }
                }
            }
        }
    }
}

fn spawn_local_server(config: &Config) -> Result<ruipc::Client, Error> {
    log::info!("Starting server...");
    let p = Command::new(crate::exe::current_exe().ok_or(Error::CurrentExe)?)
        .arg("server")
        .env("RAIN_SERVER", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(create_new_unlink(config.server_stderr_path())?)
        .spawn()?;
    log::info!("Started {}", p.id());
    log::info!("waiting for server connection");
    let start = Instant::now();
    // Wait for the socket to be created
    for _ in 0..50 {
        match ruipc::Client::connect(config.server_socket_path()) {
            Ok(stream) => {
                log::info!("connected to server after {:?}", start.elapsed());
                return Ok(stream);
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(err) => {
                return Err(err.into());
            }
        }
    }
    log::error!("timeout waiting for server to start");
    Err(Error::TimeoutWaitingForServer)
}

fn create_new_unlink(path: impl AsRef<Path>) -> std::io::Result<std::fs::File> {
    let path: &Path = path.as_ref();
    std::fs::create_dir_all(path.parent().expect("path parent"))?;
    let _ = std::fs::remove_file(path);
    std::fs::File::create_new(path)
}
