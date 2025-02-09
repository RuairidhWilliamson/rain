use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
    time::SystemTime,
};

use poison_panic::MutexExt as _;
use rain_lang::runner::cache::Cache;

use crate::{
    config::Config,
    driver::DriverImpl,
    remote::msg::{RequestWrapper, RestartReason},
};

use super::msg::{
    run::{RunProgress, RunResponse},
    Message, Request, RequestTrait,
};

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

#[expect(clippy::missing_panics_doc)]
pub fn rain_server(config: Config) -> Result<(), Error> {
    let exe_stat = crate::exe::current_exe_metadata().ok_or(Error::CurrentExe)?;
    let modified_time = exe_stat.modified()?;
    let cache = Cache::new(crate::CACHE_SIZE);
    let s = Server {
        config,
        modified_time,
        start_time: chrono::Utc::now(),
        cache,
        stats: Stats::default(),
    };
    let socket_path = s.config.server_socket_path();
    std::fs::create_dir_all(socket_path.parent().expect("path parent"))?;
    let l = crate::ipc::Listener::bind(socket_path)?;
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
    cache: Cache,
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
        let hdr: RequestWrapper = ciborium::from_reader(&mut self.stream)?;
        if hdr.modified_time != self.server.modified_time {
            log::info!("Restarting because modified time does not match");
            std::fs::remove_file(self.server.config.server_socket_path())?;
            let response = Message::RestartPls(RestartReason::RainBinaryChanged);
            ciborium::into_writer(&response, &mut self.stream)?;
            std::process::exit(0)
        }
        log::info!("Header {hdr:?}");
        let request: Request = ciborium::from_reader(std::io::Cursor::new(hdr.request))?;
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

    fn handle_request(&mut self, req: Request) -> Result<(), Error> {
        match req {
            Request::Run(req) => {
                let config = self.server.config.clone();
                let mut cache = self.server.cache.clone();
                let s = Mutex::new(self);
                let result;
                {
                    let fs = DriverImpl {
                        config,
                        prints: Mutex::default(),
                        print_handler: Some(Box::new(|m| {
                            let send_result = s
                                .plock()
                                .send_intermediate(&req, &RunProgress::Print(m.to_owned()));
                            if let Err(err) = send_result {
                                log::error!("send intermediate print: {err}");
                            }
                        })),
                        enter_handler: Some(Box::new(|m| {
                            let send_result = s
                                .plock()
                                .send_intermediate(&req, &RunProgress::EnterCall(m.to_owned()));
                            if let Err(err) = send_result {
                                log::error!("send intermediate enter call: {err}");
                            }
                        })),
                        exit_handler: Some(Box::new(|m| {
                            let send_result = s
                                .plock()
                                .send_intermediate(&req, &RunProgress::ExitCall(m.to_owned()));
                            if let Err(err) = send_result {
                                log::error!("send intermediate exit call: {err}");
                            }
                        })),
                    };
                    result =
                        crate::run(&req.root, &req.target, &mut cache, &fs).map(|v| v.to_string());
                }
                let s = s.pinto_inner();
                s.send_response(&req, &RunResponse { output: result })?;
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
                self.send_response(&req, &resp)?;
                Ok(())
            }
            Request::Shutdown(req) => {
                log::info!("Goodbye");
                self.send_response(&req, &super::msg::shutdown::Goodbye)?;
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
                self.send_response(&req, &super::msg::clean::Cleaned)?;
                std::process::exit(0);
            }
        }
    }

    fn send_intermediate<Req>(
        &mut self,
        _req: &Req,
        intermediate: &Req::Intermediate,
    ) -> Result<(), ciborium::ser::Error<std::io::Error>>
    where
        Req: RequestTrait,
    {
        let mut buf = Vec::new();
        ciborium::into_writer(&intermediate, &mut buf)?;

        let wrapped = Message::Intermediate(buf);
        ciborium::into_writer(&wrapped, &mut self.stream)
    }

    fn send_response<Req>(
        &mut self,
        _req: &Req,
        response: &Req::Response,
    ) -> Result<(), ciborium::ser::Error<std::io::Error>>
    where
        Req: RequestTrait,
    {
        let mut buf = Vec::new();
        ciborium::into_writer(&response, &mut buf)?;

        let wrapped = Message::Response(buf);
        self.server
            .stats
            .responses_sent
            .fetch_add(1, Ordering::Relaxed);
        ciborium::into_writer(&wrapped, &mut self.stream)
    }

    fn send_panic(&mut self) -> Result<(), ciborium::ser::Error<std::io::Error>> {
        let wrapped = Message::ServerPanic;
        ciborium::into_writer(&wrapped, &mut self.stream)
    }
}
