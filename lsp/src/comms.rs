use std::io::{BufRead as _, Read as _, Stdin, Stdout, Write as _};

use crate::json_rpc;

pub struct Comms {
    stdin: Stdin,
    stdout: Stdout,
}

impl Comms {
    pub fn new_stdin_stdout() -> Self {
        Self {
            stdin: std::io::stdin(),
            stdout: std::io::stdout(),
        }
    }

    pub fn send_message<M: json_rpc::Message>(&mut self, msg: &M) {
        let out = serde_json::to_string(msg).unwrap();
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
            assert_eq!(newline_buf[0], b'\n', "missing newline");
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

    pub fn receive_message<M: json_rpc::Message>(&mut self) -> M {
        let header = self.receive_header();
        let mut buf = vec![0u8; header.content_length];
        self.stdin.read_exact(&mut buf).unwrap();
        serde_json::from_slice(&buf).unwrap()
    }
}

struct Header {
    content_length: usize,
}
