use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
    time::SystemTime,
};

use rain_lang::runner::cache::Cache;

use crate::{config::Config, driver::DriverImpl, remote::msg::RestartReason};

use super::msg::{run::RunResponse, Request, RequestHeader, RequestTrait, ResponseWrapper};

#[derive(Debug)]
pub enum Error {
    CurrentExe,
    RainCacheNotADirectory,
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
    let cache = Mutex::new(Cache::new(crate::CACHE_SIZE));
    let s = Server {
        config,
        modified_time,
        start_time: chrono::Utc::now(),
        cache,
        stats: Stats::default(),
    };
    let l = crate::ipc::Listener::bind(s.config.server_socket_path())?;
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
    /// Time the rain binary was modified, used to check if we should restart the server if the file on disk is newer
    modified_time: SystemTime,
    /// Time the server was started
    start_time: chrono::DateTime<chrono::Utc>,
    // TODO: Get rid of this mutex, it is a hacky way to reuse the cache but prevents running multiple runs at once
    cache: Mutex<Cache>,
    stats: Stats,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct Stats {
    pub requests_received: AtomicUsize,
    pub responses_sent: AtomicUsize,
}

struct ClientHandler<'a> {
    server: &'a Server,
    stream: crate::ipc::Connection,
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
        self.server
            .stats
            .requests_received
            .fetch_add(1, Ordering::Relaxed);
        match std::thread::scope(|s| s.spawn(|| self.handle_request(request)).join()) {
            Err(err) => {
                log::error!("panic during handle request");
                self.send_panic()?;
                std::panic::resume_unwind(err)
            }
            Ok(Err(err)) => Err(err),
            Ok(Ok(())) => Ok(()),
        }
    }

    #[expect(clippy::unwrap_used)]
    fn handle_request(&mut self, req: Request) -> Result<(), Error> {
        match req {
            Request::Run(req) => {
                let fs = DriverImpl::new(self.server.config.clone());
                let mut cache = self.server.cache.lock().unwrap();
                let result =
                    crate::run(&req.root, &req.target, &mut cache, &fs).map(|v| v.to_string());
                let prints = fs.prints.into_inner().unwrap();
                self.send_response(
                    &req,
                    RunResponse {
                        prints,
                        output: result,
                    },
                )?;
                Ok(())
            }
            Request::Info(req) => {
                let resp = super::msg::info::InfoResponse {
                    pid: std::process::id(),
                    start_time: self.server.start_time,
                    config: self.server.config.clone(),
                    stats: super::msg::info::Stats {
                        requests_received: self
                            .server
                            .stats
                            .requests_received
                            .load(Ordering::Relaxed),
                        responses_sent: self.server.stats.responses_sent.load(Ordering::Relaxed),
                    },
                };
                self.send_response(&req, resp)?;
                Ok(())
            }
            Request::Shutdown(req) => {
                log::info!("Goodbye");
                self.send_response(&req, super::msg::shutdown::Goodbye)?;
                std::process::exit(0);
            }
            Request::Clean(req) => {
                log::info!("Cleaning");
                let clean_path = &self.server.config.base_cache_dir;
                log::info!("removing {}", clean_path.display());
                let metadata = std::fs::metadata(clean_path)?;
                if !metadata.is_dir() {
                    log::error!("failed {} is not a directory", clean_path.display());
                    return Err(Error::RainCacheNotADirectory);
                }
                std::fs::remove_dir_all(clean_path)?;
                log::info!("Goodbye");
                self.send_response(&req, super::msg::clean::Cleaned)?;
                std::process::exit(0);
            }
        }
    }

    fn send_response<Req>(
        &mut self,
        _req: &Req,
        response: Req::Response,
    ) -> Result<(), ciborium::ser::Error<std::io::Error>>
    where
        Req: RequestTrait,
    {
        let wrapped = ResponseWrapper::Response(response);
        self.server
            .stats
            .responses_sent
            .fetch_add(1, Ordering::Relaxed);
        ciborium::into_writer(&wrapped, &mut self.stream)
    }

    fn send_panic(&mut self) -> Result<(), ciborium::ser::Error<std::io::Error>> {
        // This doesn't feel safe to use generic () here but maybe it is ok
        // It might depend on the serde backend we are using
        let wrapped = ResponseWrapper::<()>::ServerPanic;
        ciborium::into_writer(&wrapped, &mut self.stream)
    }
}
