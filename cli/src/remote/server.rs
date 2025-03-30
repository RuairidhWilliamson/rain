use std::{
    collections::HashMap,
    path::Path,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Instant, SystemTime},
};

use poison_panic::MutexExt as _;
use rain_core::rain_lang::{
    afs::{dir::Dir, entry::FSEntryTrait as _, file::File},
    driver::DriverTrait as _,
    ir::Rir,
    runner::{
        Runner,
        cache::Cache,
        value::{RainTypeId, Value},
    },
};
use rain_core::{CoreError, config::Config, driver::DriverImpl};

use crate::remote::msg::{RequestWrapper, RestartReason};

use super::msg::{
    Message, Request, RequestTrait,
    run::{RunProgress, RunResponse},
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Could not get the current exe")]
    CurrentExe,
    #[error("io: {0}")]
    IO(std::io::Error),
    #[error("encode: {0}")]
    Encode(ciborium::ser::Error<std::io::Error>),
    #[error("decode: {0}")]
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
    let s = Server::new(config)?;
    let socket_path = s.config.server_socket_path();
    std::fs::create_dir_all(socket_path.parent().expect("path parent"))?;
    let mut l = ruipc::Listener::bind(socket_path)?;
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
    log::error!("server ended unexpectedly");
    Ok(())
}

struct Server {
    config: Config,
    /// Time the rain binary was modified, used to check if we should restart the server if the file on disk is newer
    modified_time: SystemTime,
    /// Time the server was started
    start_time: chrono::DateTime<chrono::Utc>,
    cache: Cache,
    stats: Stats,
    ir: Mutex<Rir>,
}

impl Server {
    fn new(config: Config) -> Result<Self, Error> {
        let exe_stat = crate::exe::current_exe_metadata().ok_or(Error::CurrentExe)?;
        let modified_time = exe_stat.modified()?;
        let cache = Cache::new(rain_core::rain_lang::runner::cache::CACHE_SIZE);
        Ok(Self {
            config,
            modified_time,
            start_time: chrono::Utc::now(),
            cache,
            stats: Stats::default(),
            ir: Mutex::new(Rir::new()),
        })
    }
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct Stats {
    pub requests_received: AtomicUsize,
    pub responses_sent: AtomicUsize,
}

struct ClientHandler<'a> {
    server: &'a Server,
    stream: ruipc::Connection,
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
            Request::Run(req) => self.run(req),
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
                self.send_response(req, &resp)?;
                Ok(())
            }
            Request::Inspect(req) => {
                let cache_size = self.server.cache.len();
                let entries = self.server.cache.inspect_all();
                self.send_response(
                    req,
                    &super::msg::inspect::InspectResponse {
                        cache_size,
                        entries,
                    },
                )?;
                Ok(())
            }
            Request::Shutdown(req) => {
                log::info!("Goodbye");
                self.send_response(req, &super::msg::shutdown::Goodbye)?;
                std::process::exit(0);
            }
            Request::Clean(req) => self.clean(req),
        }
    }

    fn run(&mut self, req: super::msg::run::RunRequest) -> Result<(), Error> {
        let config = self.server.config.clone();
        let cache = self.server.cache.clone();
        let mut ir = self.server.ir.plock();
        let s = Mutex::new(self);
        let start = Instant::now();
        let result = run_inner(&req, config, cache, &s, &mut ir);
        let s = s.pinto_inner();
        s.send_response(
            req,
            &RunResponse {
                output: result,
                elapsed: start.elapsed(),
            },
        )?;
        Ok(())
    }

    fn clean(&mut self, req: super::msg::clean::CleanRequest) -> Result<(), Error> {
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
        self.send_response(req, &super::msg::clean::Cleaned(sizes))?;
        std::process::exit(0);
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
        _req: Req,
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

fn run_inner(
    req: &super::msg::run::RunRequest,
    config: Config,
    cache: Cache,
    s: &Mutex<&mut ClientHandler<'_>>,
    ir: &mut Rir,
) -> Result<String, CoreError> {
    let driver = DriverImpl {
        config,
        prints: Mutex::default(),
        print_handler: Some(Box::new(|m| {
            let send_result = s
                .plock()
                .send_intermediate(req, &RunProgress::Print(m.to_owned()));
            if let Err(err) = send_result {
                log::error!("send intermediate print: {err}");
            }
        })),
        enter_handler: Some(Box::new(|m| {
            let send_result = s
                .plock()
                .send_intermediate(req, &RunProgress::EnterCall(m.to_owned()));
            if let Err(err) = send_result {
                log::error!("send intermediate enter call: {err}");
            }
        })),
        exit_handler: Some(Box::new(|m| {
            let send_result = s
                .plock()
                .send_intermediate(req, &RunProgress::ExitCall(m.to_owned()));
            if let Err(err) = send_result {
                log::error!("send intermediate exit call: {err}");
            }
        })),
    };

    run_core(req, cache, &driver, ir).map(|v| match v.rain_type_id() {
        RainTypeId::Unit => String::new(),
        RainTypeId::Dir if req.resolve => {
            if let Some(d) = v.downcast_ref::<Dir>() {
                driver.resolve_fs_entry(d.inner()).display().to_string()
            } else {
                unreachable!()
            }
        }
        RainTypeId::File if req.resolve => {
            if let Some(f) = v.downcast_ref::<File>() {
                driver.resolve_fs_entry(f.inner()).display().to_string()
            } else {
                unreachable!()
            }
        }
        _ => v.to_string(),
    })
}

fn run_core(
    req: &super::msg::run::RunRequest,
    mut cache: Cache,
    driver: &DriverImpl<'_>,
    ir: &mut Rir,
) -> Result<Value, CoreError> {
    let path = &req.root;
    let declaration: &str = &req.target;
    let file = File::new_local(path.as_ref()).map_err(|err| CoreError::Other(err.to_string()))?;
    let path = driver.resolve_fs_entry(file.inner());
    let src = std::fs::read_to_string(&path).map_err(|err| CoreError::Other(err.to_string()))?;
    let module = rain_core::rain_lang::ast::parser::parse_module(&src);
    let mid = ir
        .insert_module(file, src, module)
        .map_err(|err| CoreError::LangError(Box::new(err.resolve_ir(ir).into_owned())))?;
    let main = ir
        .resolve_global_declaration(mid, declaration)
        .ok_or_else(|| CoreError::Other(String::from("declaration does not exist")))?;
    let mut runner = Runner::new(ir, &mut cache, driver);
    let value = runner
        .evaluate_and_call(main)
        .map_err(|err| CoreError::LangError(Box::new(err.resolve_ir(runner.ir).into_owned())))?;
    Ok(value)
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
