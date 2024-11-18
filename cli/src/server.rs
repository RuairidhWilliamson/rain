use std::{
    os::unix::net::{UnixListener, UnixStream},
    time::SystemTime,
};

use rain_core::config::Config;

use crate::msg::{Request, RequestHeader, Response, ResponseWrapper};

pub fn rain_server(config: Config) -> Result<(), ()> {
    let exe_stat = std::fs::metadata(std::env::current_exe().unwrap()).unwrap();
    let modified_time = exe_stat.modified().unwrap();
    let s = Server {
        config,
        modified_time,
    };
    let l = UnixListener::bind(s.config.server_socket_path()).unwrap();
    for stream in l.incoming() {
        match stream {
            Ok(stream) => {
                log::info!("got a stream {stream:?}");
                ClientHandler { server: &s, stream }.handle_client();
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
    fn handle_client(mut self) {
        let hdr: RequestHeader = ciborium::from_reader(&mut self.stream).unwrap();
        if hdr.modified_time != self.server.modified_time {
            log::info!("Restarting because modified time does not match");
            std::fs::remove_file(self.server.config.server_socket_path()).unwrap();
            let response = ResponseWrapper::RestartRequest;
            ciborium::into_writer(&response, &mut self.stream).unwrap();
            std::process::exit(0)
        }
        let request: Request = ciborium::from_reader(&mut self.stream).unwrap();
        log::info!("Header {hdr:?}");
        log::info!("Request {request:?}");
        self.handle_request(&request);
    }

    fn handle_request(self, req: &Request) {
        match req {
            Request::Info => self.send_response(Response::Success),
            Request::Shutdown => {
                log::info!("Goodbye");
                self.send_response(Response::Success);
                std::process::exit(0);
            }
        }
    }

    fn send_response(mut self, response: impl Into<ResponseWrapper>) {
        ciborium::into_writer(&response.into(), &mut self.stream).unwrap();
    }
}
