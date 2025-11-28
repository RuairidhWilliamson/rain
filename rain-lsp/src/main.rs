#![allow(
    clippy::unwrap_used,
    clippy::dbg_macro,
    clippy::print_stderr,
    clippy::print_stdout
)]

mod json_rpc;

use std::{
    io::{BufRead as _, Read as _, Stdin, Stdout, Write as _},
    process::ExitCode,
};

fn main() -> ExitCode {
    let mut comms = Comms {
        stdin: std::io::stdin(),
        stdout: std::io::stdout(),
    };
    let initialize = comms.receive_message::<json_rpc::Request<lsp_types::InitializeParams>>();
    comms.send_message(&initialize.ok_response(lsp_types::InitializeResult {
        capabilities: lsp_types::ServerCapabilities {
            ..Default::default()
        },
        server_info: None,
    }));
    comms.send_message(&json_rpc::Notification::new("initialized", ()));

    loop {
        let message = comms.receive_message::<json_rpc::Request<serde_json::Value>>();
        match message.method.as_str() {
            "initialized" => {}
            "shutdown" => {
                comms.send_message(&message.ok_response(()));
            }
            "exit" => return ExitCode::SUCCESS,
            _ => {
                dbg!(&message);
                if !message.id.is_null() {
                    comms.send_message(&message.error_response(json_rpc::ResponseError {
                        code: json_rpc::METHOD_NOT_FOUND,
                        message: String::from("unknown method"),
                        data: None,
                    }));
                }
            }
        }
    }
}

struct Comms {
    stdin: Stdin,
    stdout: Stdout,
}

impl Comms {
    fn send_message<M: json_rpc::Message>(&mut self, msg: &M) {
        let out = serde_json::to_string(msg).unwrap();
        eprintln!("{out}");
        write!(self.stdout, "Content-Length: {}\r\n\r\n{out}", out.len()).unwrap();
        self.stdout.flush().unwrap();
    }

    fn receive_header(&mut self) -> Header {
        let mut h = Header { content_length: 0 };
        let mut buf = Vec::new();
        loop {
            self.stdin.lock().read_until(b'\r', &mut buf).unwrap();
            let mut newline_buf = [0u8; 1];
            self.stdin.read_exact(&mut newline_buf[..]).unwrap();
            if newline_buf[0] != b'\n' {
                panic!("missing newline");
            }
            let s = std::str::from_utf8(&buf[..buf.len() - 1]).unwrap();
            if s.is_empty() {
                break;
            }
            let content_length: usize = s
                .strip_prefix("Content-Length: ")
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            h.content_length = content_length;
            buf.clear();
        }
        h
    }

    fn receive_message<M: json_rpc::Message>(&mut self) -> M {
        let header = self.receive_header();
        let mut buf = vec![0u8; header.content_length];
        self.stdin.read_exact(&mut buf).unwrap();
        serde_json::from_slice(&buf).unwrap()
    }
}

struct Header {
    content_length: usize,
}
