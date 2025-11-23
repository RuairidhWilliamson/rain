use std::{
    borrow::Cow,
    io::{BufRead as _, Read, Stdin, Stdout, Write},
    process::ExitCode,
};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Request<T> {
    id: serde_json::Value,
    jsonrpc: Cow<'static, str>,
    method: String,
    params: Option<T>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Notification<T> {
    method: String,
    params: Option<T>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Response<T> {
    id: serde_json::Value,
    jsonrpc: Cow<'static, str>,
    result: Option<T>,
    error: Option<ResponseError>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ResponseError {
    code: i64,
    message: String,
    data: Option<serde_json::Value>,
}

fn main() -> ExitCode {
    let mut comms = Comms {
        stdin: std::io::stdin(),
        stdout: std::io::stdout(),
    };
    let initialize = comms.receive_message::<Request<lsp_types::InitializeParams>>();
    comms.send_message(&Response::<lsp_types::InitializeResult> {
        id: initialize.id,
        jsonrpc: "2.0".into(),
        result: Some(lsp_types::InitializeResult {
            capabilities: lsp_types::ServerCapabilities {
                // hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
                ..Default::default()
            },
            server_info: None,
        }),
        error: None,
    });
    comms.send_message(&Notification::<()> {
        method: String::from("initialized"),
        params: None,
    });

    loop {
        let message = comms.receive_message::<serde_json::Value>();
        let method = message.get("method").unwrap().as_str().unwrap();
        match method {
            "initialized" => {}
            "shutdown" => {
                let shutdown: Request<()> = serde_json::from_value(message).unwrap();
                comms.send_message(&Response {
                    id: shutdown.id,
                    jsonrpc: "2.0".into(),
                    result: Some(serde_json::Value::Null),
                    error: None,
                });
                comms.send_message(&Notification::<()> {
                    method: String::from("exit"),
                    params: None,
                });
            }
            "exit" => return ExitCode::SUCCESS,
            _ => {
                dbg!(message);
            }
        }
    }
}

struct Comms {
    stdin: Stdin,
    stdout: Stdout,
}

impl Comms {
    fn send_message<T: serde::Serialize>(&mut self, msg: &T) {
        let out = serde_json::to_string(msg).unwrap();
        eprintln!("{out}");
        write!(self.stdout, "Content-Length: {}\r\n\r\n{out}", out.len()).unwrap();
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

    fn receive_message<T: for<'de> serde::Deserialize<'de>>(&mut self) -> T {
        let header = self.receive_header();
        let mut buf = vec![0u8; header.content_length];
        self.stdin.read_exact(&mut buf).unwrap();
        serde_json::from_slice(&buf).unwrap()
    }
}

struct Header {
    content_length: usize,
}
