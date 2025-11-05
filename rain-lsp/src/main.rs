use std::{
    borrow::Cow,
    env::temp_dir,
    io::{Read, Stdin, Stdout, Write},
};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Request<T> {
    id: Option<serde_json::Value>,
    jsonrpc: Cow<'static, str>,
    method: String,
    params: T,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Response<T> {
    id: serde_json::Value,
    jsonrpc: Cow<'static, str>,
    result: T,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ResponseErr {
    id: serde_json::Value,
    jsonrpc: Cow<'static, str>,
    error: serde_json::Value,
}

fn main() {
    let log_path = temp_dir().join("rain-lsp.log");
    let mut log_file = std::fs::File::create(log_path).unwrap();
    let mut comms = Comms {
        stdin: std::io::stdin(),
        stdout: std::io::stdout(),
    };
    let initialize = comms.receive_message::<Request<lsp_types::InitializeParams>>();
    writeln!(log_file, "{initialize:#?}").unwrap();
    comms.send_response(&Response::<lsp_types::InitializeResult> {
        id: initialize.id.unwrap(),
        jsonrpc: "2.0".into(),
        result: lsp_types::InitializeResult {
            capabilities: lsp_types::ServerCapabilities::default(),
            server_info: None,
        },
    });

    loop {
        let message = comms.receive_message::<Request<serde_json::Value>>();
        writeln!(log_file, "{message:#?}").unwrap();
    }
}

struct Comms {
    stdin: Stdin,
    stdout: Stdout,
}

impl Comms {
    fn send_response<T: serde::Serialize>(&mut self, response: &Response<T>) {
        let out = serde_json::to_string(response).unwrap();
        write!(self.stdout, "Content-Length: {}\r\n\r\n", out.len()).unwrap();
        write!(self.stdout, "{out}\r\n").unwrap();
    }

    fn receive_header(&mut self) -> Header {
        let mut s = String::new();
        self.stdin.read_line(&mut s).unwrap();
        let content_length: usize = dbg!(&s)
            .strip_prefix("Content-Length: ")
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        self.stdin.read_line(&mut s).unwrap();
        Header { content_length }
    }

    fn receive_message<T: for<'de> serde::Deserialize<'de>>(&mut self) -> T {
        let header = self.receive_header();
        let mut buf = vec![0u8; header.content_length];
        self.stdin.read_exact(&mut buf).unwrap();
        let buf = String::from_utf8(buf).unwrap();
        serde_json::from_str(&buf).unwrap()
    }
}

struct Header {
    content_length: usize,
}
