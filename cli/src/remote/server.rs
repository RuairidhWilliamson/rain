use std::{
    collections::HashMap,
    fs, io, panic,
    path::Path,
    process,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc::{Receiver, SyncSender, sync_channel},
    },
    thread,
    time::{Instant, SystemTime},
};

use poison_panic::MutexExt as _;
use rain_core::{
    CoreError,
    cache::{
        Cache, CacheStats,
        persistent::{PersistCache, PersistCacheError},
    },
    config::Config,
    driver::DriverImpl,
    rain_lang::{
        driver::FSTrait as _,
        ir::Rir,
        runner::{cache::CacheTrait as _, dep_list::DepList, value::Value},
    },
};

use crate::remote::msg::{
    Request, RequestTrait, RequestWrapper, RestartReason, ServerMessage,
    prune::Pruned,
    run::{RunProgress, RunResponse},
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not get the current exe")]
    CurrentExe,
    #[error("server graceful exit")]
    GracefulExit,
    #[error("io: {0}")]
    IO(io::Error),
    #[error("encode: {0}")]
    Encode(ciborium::ser::Error<io::Error>),
    #[error("decode: {0}")]
    Decode(ciborium::de::Error<io::Error>),
    #[error("serde: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("cache: {0}")]
    PersistentCache(#[from] PersistCacheError),
    #[error("client disconnected")]
    ClientDisconnected,
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::IO(err)
    }
}

impl From<ciborium::ser::Error<io::Error>> for Error {
    fn from(err: ciborium::ser::Error<io::Error>) -> Self {
        Self::Encode(err)
    }
}

impl From<ciborium::de::Error<io::Error>> for Error {
    fn from(err: ciborium::de::Error<io::Error>) -> Self {
        Self::Decode(err)
    }
}

pub fn rain_server(config: Config) -> Result<(), Error> {
    log::info!("starting cli server");
    let mut s = Server::new(config)?;
    let socket_path = s.config.server_socket_path();
    fs::create_dir_all(socket_path.parent().expect("path parent"))?;
    let mut l = ruipc::Listener::bind(socket_path)?;
    for stream in l.incoming() {
        match stream {
            Ok(connection) => {
                log::info!("got a stream {connection:?}");
                let result = ClientHandler {
                    server: &mut s,
                    stream: IpcMsgConnection { connection },
                }
                .handle_client();
                match result {
                    Ok(()) => (),
                    Err(Error::GracefulExit) => process::exit(0),
                    Err(err) => return Err(err),
                }
            }
            Err(err) => {
                log::error!("unix listener error: {err}");
            }
        }
    }
    log::error!("server ended unexpectedly");
    Ok(())
}

pub struct Server {
    config: Config,
    /// Time the rain binary was modified, used to check if we should restart the server if the file on disk is newer
    modified_time: SystemTime,
    /// Time the server was started
    start_time: chrono::DateTime<chrono::Utc>,
    cache: Option<PersistCache>,
    cache_stats: CacheStats,
    stats: Stats,
}

impl Server {
    pub fn new(config: Config) -> Result<Self, Error> {
        let exe_stat = crate::exe::current_exe_metadata().ok_or(Error::CurrentExe)?;
        let modified_time = exe_stat.modified()?;
        let cache = PersistCache::load(&config.cache_json_path())
            .inspect_err(|err| {
                log::info!("failed to load persist cache: {err}");
            })
            .ok();
        Ok(Self {
            config,
            modified_time,
            start_time: chrono::Utc::now(),
            cache,
            cache_stats: Default::default(),
            stats: Stats::default(),
        })
    }
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct Stats {
    pub requests_received: AtomicUsize,
    pub responses_sent: AtomicUsize,
}

pub trait MsgConnection: Send {
    fn send(&mut self, request: ServerMessage) -> Result<(), Error>;
    fn receive(&mut self) -> Result<RequestWrapper, Error>;
}

pub struct IpcMsgConnection {
    connection: ruipc::Connection,
}

impl MsgConnection for IpcMsgConnection {
    fn send(&mut self, request: ServerMessage) -> Result<(), Error> {
        ciborium::into_writer(&request, &mut self.connection)?;
        Ok(())
    }

    fn receive(&mut self) -> Result<RequestWrapper, Error> {
        let request = ciborium::from_reader(&mut self.connection)?;
        Ok(request)
    }
}

pub struct InternalMsgConnection {
    pub tx: SyncSender<ServerMessage>,
    pub rx: Receiver<RequestWrapper>,
}

impl InternalMsgConnection {
    pub fn new() -> (Self, SyncSender<RequestWrapper>, Receiver<ServerMessage>) {
        let (tx1, rx1) = sync_channel(1);
        let (tx2, rx2) = sync_channel(1);
        (Self { tx: tx1, rx: rx2 }, tx2, rx1)
    }
}

impl MsgConnection for InternalMsgConnection {
    fn send(&mut self, request: ServerMessage) -> Result<(), Error> {
        self.tx.send(request).map_err(|_| Error::ClientDisconnected)
    }

    fn receive(&mut self) -> Result<RequestWrapper, Error> {
        self.rx.recv().map_err(|_| Error::ClientDisconnected)
    }
}

pub struct ClientHandler<'a, C> {
    pub server: &'a mut Server,
    pub stream: C,
}

impl<C: MsgConnection> ClientHandler<'_, C> {
    pub fn handle_client(mut self) -> Result<(), Error> {
        let RequestWrapper { header, request } = self.stream.receive()?;
        if header.exe != crate::exe::current_exe().ok_or(Error::CurrentExe)? {
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
        let request: Request = ciborium::from_reader(io::Cursor::new(request))?;
        log::info!("Request {request:?}");
        self.server
            .stats
            .requests_received
            .fetch_add(1, Ordering::Relaxed);
        let mut ir = Rir::new();
        let mut cache = Cache::new(
            self.server
                .cache
                .take()
                .map(|c| c.depersist(&self.server.config, &self.server.cache_stats, &mut ir))
                .unwrap_or_default(),
        );

        match thread::scope(|s| {
            thread::Builder::new()
                .name(String::from("handle_request"))
                .spawn_scoped(s, || self.handle_request(&mut cache, &mut ir, request))
                .expect("spawn thread")
                .join()
        }) {
            Err(err) => {
                log::error!("panic during handle request");
                self.send_panic()?;
                panic::resume_unwind(err)
            }
            Ok(Err(err)) => Err(err),
            Ok(Ok(())) => {
                let persistent_cache =
                    PersistCache::persist(&cache.core.plock(), &cache.stats, &ir);
                persistent_cache.save(&self.server.config.cache_json_path())?;
                self.server.cache = Some(persistent_cache);
                log::info!("cache stats {:#?}", self.server.cache_stats);
                Ok(())
            }
        }
    }

    fn restart(&mut self) -> Result<(), Error> {
        fs::remove_file(self.server.config.server_socket_path())?;
        let response = ServerMessage::RestartPls(RestartReason::RainBinaryChanged);
        self.stream.send(response)?;
        Err(Error::GracefulExit)
    }

    fn handle_request(
        &mut self,
        cache: &mut Cache,
        ir: &mut Rir,
        req: Request,
    ) -> Result<(), Error> {
        match req {
            Request::Run(req) => self.run(cache, ir, req),
            Request::Info(req) => {
                let resp = super::msg::info::InfoResponse {
                    pid: process::id(),
                    start_time: self.server.start_time,
                    config: self.server.config.clone(),
                    stats: super::msg::info::Stats {
                        requests_received: self
                            .server
                            .stats
                            .requests_received
                            .load(Ordering::Relaxed),
                        responses_sent: self.server.stats.responses_sent.load(Ordering::Relaxed),
                        cache_size: cache.len(),
                    },
                };
                self.send_response(req, &resp)?;
                Ok(())
            }
            Request::Inspect(req) => {
                let cache_size = cache.len();
                let entries = cache.inspect_all();
                self.send_response(
                    req,
                    &super::msg::cache_inspect::CacheInspectResponse {
                        cache_size,
                        entries,
                    },
                )?;
                Ok(())
            }
            Request::Shutdown(req) => {
                log::info!("Goodbye");
                self.send_response(req, &super::msg::shutdown::Goodbye)?;
                Err(Error::GracefulExit)
            }
            Request::Clean(req) => self.clean(cache, req),
            Request::Prune(req) => self.prune(cache, req),
        }
    }

    fn run(
        &mut self,
        cache: &mut Cache,
        ir: &mut Rir,
        req: super::msg::run::RunRequest,
    ) -> Result<(), Error> {
        cache.verification = req.verification;
        let config = self.server.config.clone();
        let s = Mutex::new(self);
        let start = Instant::now();
        let (result, deps) = run_inner(&req, config, cache, &s, ir);
        let s = s.pinto_inner();
        s.send_response(
            req,
            &RunResponse {
                output: result,
                deps,
                elapsed: start.elapsed(),
            },
        )?;
        Ok(())
    }

    fn clean(&mut self, cache: &Cache, req: super::msg::clean::CleanRequest) -> Result<(), Error> {
        log::info!("Cleaning");
        cache.clean();
        let clean_paths = &[
            &self.server.config.base_cache_dir,
            &self.server.config.base_generated_dir,
            &self.server.config.base_data_dir,
            &self.server.config.base_run_dir,
        ];
        let mut sizes = HashMap::new();
        for p in clean_paths {
            log::info!("removing {}", p.display());
            let metadata = match fs::metadata(p) {
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
        Err(Error::GracefulExit)
    }

    fn prune(&mut self, cache: &Cache, req: super::msg::prune::PruneRequest) -> Result<(), Error> {
        let guard = cache.core.plock();
        let pruned = guard.prune_generated_areas(&self.server.config)?;
        self.send_response(
            req,
            &Pruned {
                size: pruned.size,
                errors: pruned.errors,
            },
        )?;
        Ok(())
    }

    fn send_intermediate<Req>(
        &mut self,
        _req: &Req,
        intermediate: &Req::Intermediate,
    ) -> Result<(), Error>
    where
        Req: RequestTrait,
    {
        let mut buf = Vec::new();
        ciborium::into_writer(&intermediate, &mut buf)?;

        let wrapped = ServerMessage::Intermediate(buf);
        self.stream.send(wrapped)
    }

    fn send_response<Req>(&mut self, _req: Req, response: &Req::Response) -> Result<(), Error>
    where
        Req: RequestTrait,
    {
        let mut buf = Vec::new();
        ciborium::into_writer(&response, &mut buf)?;

        let wrapped = ServerMessage::Response(buf);
        self.server
            .stats
            .responses_sent
            .fetch_add(1, Ordering::Relaxed);
        self.stream.send(wrapped)
    }

    fn send_panic(&mut self) -> Result<(), Error> {
        let wrapped = ServerMessage::ServerPanic;
        self.stream.send(wrapped)
    }
}

fn run_inner<C: MsgConnection>(
    req: &super::msg::run::RunRequest,
    config: Config,
    cache: &Cache,
    s: &Mutex<&mut ClientHandler<'_, C>>,
    ir: &mut Rir,
) -> (Result<String, CoreError>, DepList) {
    let mut driver = DriverImpl::new(config);
    driver.custom_config = req
        .custom_config
        .iter()
        .map(|(k, v)| (k.clone(), Arc::new(v.clone())))
        .collect();
    driver.print_handler = Some(Box::new(|m| {
        let send_result = s
            .plock()
            .send_intermediate(req, &RunProgress::Print(m.to_owned()));
        if let Err(err) = send_result {
            log::error!("send intermediate print: {err}");
        }
    }));
    driver.enter_handler = Some(Box::new(|m| {
        let send_result = s
            .plock()
            .send_intermediate(req, &RunProgress::EnterCall(m.to_owned()));
        if let Err(err) = send_result {
            log::error!("send intermediate enter call: {err}");
        }
    }));
    driver.exit_handler = Some(Box::new(|m| {
        let send_result = s
            .plock()
            .send_intermediate(req, &RunProgress::ExitCall(m.to_owned()));
        if let Err(err) = send_result {
            log::error!("send intermediate exit call: {err}");
        }
    }));
    if let Some(host_override) = &req.host_override {
        driver.host_triple = host_override.to_owned().into();
    }

    run_core(req, cache, &driver, ir)
}

fn run_core(
    super::msg::run::RunRequest {
        root,
        target,
        args,
        resolve,
        offline,
        seal,
        host_override: _,
        custom_config: _,
        verification: _,
        unused,
        no_exec,
    }: &super::msg::run::RunRequest,
    cache: &Cache,
    driver: &DriverImpl<'_>,
    ir: &mut Rir,
) -> (Result<String, CoreError>, DepList) {
    let mut runner = rain_core::new_runner(ir, cache, driver);
    runner.offline = *offline;
    runner.seal = *seal;
    runner.check_unused = *unused;
    runner.no_exec = *no_exec;
    let mut deps = DepList::new();
    let mid = match rain_core::insert_local_module(&mut runner, root) {
        Ok(mid) => mid,
        Err(err) => return (Err(err), deps),
    };
    let result = rain_core::evaluate_and_call_chain(&mut runner, mid, &mut deps, target, args);
    (
        result.map(|v| match v {
            Value::Unit => String::new(),
            Value::GeneratedDir(d) if *resolve => driver
                .resolve_fs_entry(d.fsinner().into())
                .display()
                .to_string(),
            Value::GeneratedFile(f) if *resolve => driver
                .resolve_fs_entry(f.fsinner().into())
                .display()
                .to_string(),
            _ => format!("{v}"),
        }),
        deps,
    )
}

fn remove_recursive(path: &Path) -> io::Result<u64> {
    let metadata = fs::symlink_metadata(path)?;
    let filetype = metadata.file_type();
    if filetype.is_symlink() {
        fs::remove_file(path)?;
        return Ok(metadata.len());
    }
    remove_dir_all_recursive(path)
}

fn remove_dir_all_recursive(path: &Path) -> io::Result<u64> {
    let mut size = 0;
    for child in fs::read_dir(path)? {
        let child = child?;
        if child.file_type()?.is_dir() {
            size += remove_dir_all_recursive(&child.path())?;
        } else {
            size += child.metadata()?.len();
            fs::remove_file(child.path())?;
        }
    }
    fs::remove_dir(path)?;
    Ok(size)
}
