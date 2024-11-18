use std::{os::unix::net::UnixStream, process::Stdio, time::Duration};

use rain_core::config::Config;

use crate::msg::{Request, RequestHeader, Response, ResponseWrapper};

pub fn make_request_or_start(config: Config, request: &Request) -> std::io::Result<Response> {
    log::info!("Connecting");
    let stream: UnixStream;
    match UnixStream::connect(config.server_socket_path()) {
        Ok(s) => {
            stream = s;
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            log::info!("No socket at path");
            stream = start_server(&config)?;
        }
        Err(err) if err.kind() == std::io::ErrorKind::ConnectionRefused => {
            log::info!("Found stale socket, removing...");
            std::fs::remove_file(config.server_socket_path())?;
            stream = start_server(&config)?;
        }
        Err(err) => {
            return Err(err);
        }
    }
    let response = make_request(stream, config.clone(), request)?;
    match response {
        ResponseWrapper::RestartRequest => {
            let stream = start_server(&config)?;
            match make_request(stream, config, request)? {
                ResponseWrapper::Response(resp) => Ok(resp),
                ResponseWrapper::RestartRequest => {
                    panic!("second restart request")
                }
            }
        }
        ResponseWrapper::Response(resp) => Ok(resp),
    }
}

fn start_server(config: &Config) -> std::io::Result<UnixStream> {
    log::info!("Starting server...");
    let p = std::process::Command::new(std::env::current_exe()?)
        .env("RAIN_SERVER", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(config.server_stderr_path()).unwrap())
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
                return Err(err);
            }
        }
    }
    todo!("timeout waiting for server to start");
}

fn make_request(
    mut stream: UnixStream,
    config: Config,
    request: &Request,
) -> std::io::Result<ResponseWrapper> {
    let hdr = RequestHeader {
        config,
        modified_time: std::fs::metadata(std::env::current_exe().unwrap())
            .unwrap()
            .modified()
            .unwrap(),
    };
    ciborium::into_writer(&hdr, &mut stream).unwrap();
    ciborium::into_writer(request, &mut stream).unwrap();
    let response: ResponseWrapper = ciborium::from_reader(&mut stream).unwrap();
    log::info!("Got repsonse {response:?}");
    Ok(response)
}
