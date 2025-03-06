use std::{
    collections::HashMap,
    path::Path,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
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
    Message, Request, RequestTrait,
    run::{RunProgress, RunResponse},
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
        let RequestWrapper { header, request } = ciborium::from_reader(&mut self.stream)?;
        if header.exe != std::fs::read_link("/proc/self/exe")? {
            log::info!("Restarting because exe symlink changed");
            return self.restart();
        }
        if header.modified_time != self.server.modified_time {
            log::info!("Restarting because modified time does not match");
            return self.restart();
        }
        if header.config != self.server.config {
            log::info!("Restarting because config does not match");
            return self.restart();
        }
        log::info!("Header {header:?}");
        let request: Request = ciborium::from_reader(std::io::Cursor::new(request))?;
        log::info!("Request {request:?}");
        self.server
            .stats
            .requests_received
            .fetch_add(1, Ordering::Relaxed);
        match std::thread::scope(|s| {
            std::thread::Builder::new()
                .name(String::from("handle_request"))
                .spawn_scoped(s, || self.handle_request(request))
                .expect("spawn thread")
                .join()
        }) {
            Err(err) => {
                log::error!("panic during handle request");
                self.send_panic()?;
                std::panic::resume_unwind(err)
            }
            Ok(Err(err)) => Err(err),
            Ok(Ok(())) => Ok(()),
        }
    }

    fn restart(&mut self) -> Result<(), Error> {
        std::fs::remove_file(self.server.config.server_socket_path())?;
        let response = Message::RestartPls(RestartReason::RainBinaryChanged);
        ciborium::into_writer(&response, &mut self.stream)?;
        std::process::exit(0)
    }

    fn handle_request(&mut self, req: Request) -> Result<(), Error> {
        match req {
            Request::Run(req) => {
                let config = self.server.config.clone();
                let mut cache = self.server.cache.clone();
                let s = Mutex::new(self);
                let result;
                {
                    let driver = DriverImpl {
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
                    result = crate::run(&req.root, &req.target, &mut cache, &driver)
                        .map(|v| v.to_string());
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
                        cache_size: self.server.cache.len(),
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
                let clean_paths = &[
                    &self.server.config.base_cache_dir,
                    &self.server.config.base_generated_dir,
                    &self.server.config.base_data_dir,
                    &self.server.config.base_run_dir,
                ];
                let mut sizes = HashMap::new();
                for p in clean_paths {
                    log::info!("removing {}", p.display());
                    let metadata = match std::fs::metadata(p) {
                        Err(err) => {
                            log::error!("failed {}: {err}", p.display());
                            continue;
                        }
                        Ok(metadata) => metadata,
                    };
                    if !metadata.is_dir() {
                        log::error!("failed {} is not a directory", p.display());
                        continue;
                    }
                    let size = remove_recursive(p)?;
                    sizes.insert((*p).clone(), size);
                }
                log::info!("Goodbye");
                self.send_response(&req, &super::msg::clean::Cleaned(sizes))?;
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

fn remove_recursive(path: &Path) -> std::io::Result<u64> {
    let metadata = std::fs::symlink_metadata(path)?;
    let filetype = metadata.file_type();
    if filetype.is_symlink() {
        std::fs::remove_file(path)?;
        return Ok(metadata.len());
    }
    remove_dir_all_recursive(path)
}

fn remove_dir_all_recursive(path: &Path) -> std::io::Result<u64> {
    let mut size = 0;
    for child in std::fs::read_dir(path)? {
        let child = child?;
        if child.file_type()?.is_dir() {
            size += remove_dir_all_recursive(&child.path())?;
        } else {
            size += child.metadata()?.len();
            std::fs::remove_file(child.path())?;
        }
    }
    std::fs::remove_dir(path)?;
    Ok(size)
}
