#![allow(
    clippy::unwrap_used,
    clippy::dbg_macro,
    clippy::print_stderr,
    clippy::print_stdout
)]

mod json_rpc;

use std::{
    collections::HashMap,
    io::{BufRead as _, Read as _, Stdin, Stdout, Write as _},
    process::ExitCode,
};

use lsp_types::{DidOpenTextDocumentParams, Hover, HoverParams, HoverProviderCapability};
use rain_lang::local_span::LocalSpan;

fn main() -> ExitCode {
    let mut comms = Comms {
        stdin: std::io::stdin(),
        stdout: std::io::stdout(),
    };
    let initialize = comms.receive_message::<json_rpc::Request<lsp_types::InitializeParams>>();
    comms.send_message(&initialize.ok_response(lsp_types::InitializeResult {
        capabilities: lsp_types::ServerCapabilities {
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            ..Default::default()
        },
        server_info: None,
    }));
    comms.send_message(&json_rpc::Notification::<()>::new("initialized", None));

    let mut text_documents = HashMap::new();

    loop {
        let message = comms.receive_message::<json_rpc::Request<serde_json::Value>>();
        match message.method.as_ref() {
            "initialized" => {}
            "shutdown" => {
                comms.send_message(&message.ok_response(()));
            }
            "textDocument/didOpen" => {
                let message = message
                    .cast_params::<DidOpenTextDocumentParams>()
                    .unwrap()
                    .assert_notification();
                let params = message.params.unwrap();
                text_documents.insert(params.text_document.uri, params.text_document.text);
            }
            "textDocument/hover" => {
                let message = message.cast_params::<HoverParams>().unwrap();
                let params = message.params.clone().unwrap();
                let src = text_documents
                    .get(&params.text_document_position_params.text_document.uri)
                    .unwrap();
                let span = LocalSpan::byte_from_line_colz(
                    src,
                    params.text_document_position_params.position.line as usize,
                    params.text_document_position_params.position.character as usize,
                )
                .unwrap();
                let module = rain_lang::ast::parser::parse_module(src).unwrap();
                let node = module.find_node_by_span(span).unwrap();
                let display = module.display_node(src, node);
                let node_span = module.span(node);
                let (start_line, start_col) = node_span.start_line_colz(src);
                let (end_line, end_col) = node_span.end_line_colz(src);

                comms.send_message(&message.ok_response(Hover {
                    contents: lsp_types::HoverContents::Scalar(lsp_types::MarkedString::String(
                        display,
                    )),
                    range: Some(lsp_types::Range {
                        start: lsp_types::Position {
                            line: start_line as u32,
                            character: start_col as u32,
                        },
                        end: lsp_types::Position {
                            line: end_line as u32,
                            character: end_col as u32,
                        },
                    }),
                }));
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
