use std::{collections::HashMap, process::ExitCode};

use lsp_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, Hover, HoverParams, HoverProviderCapability,
    PublishDiagnosticsParams, TextDocumentSyncKind,
};

use crate::{
    comms::Comms,
    json_rpc::{self, Notification, Request},
};

pub struct Server {
    comms: Comms,
    text_documents: HashMap<String, TextDocument>,
}

struct TextDocument {
    uri: lsp_types::Uri,
    version: i32,
    source: String,
    tree: tree_sitter::Tree,
}

impl TextDocument {
    fn publish_diagnostics(&self) -> Notification<PublishDiagnosticsParams> {
        Notification::new(
            "textDocument/publishDiagnostics",
            Some(PublishDiagnosticsParams {
                uri: self.uri.clone(),
                version: Some(self.version),
                diagnostics: self.diagnostics().collect(),
            }),
        )
    }

    fn diagnostics(&self) -> impl Iterator<Item = Diagnostic> {
        tree_errors(&self.tree).map(|node| Diagnostic {
            range: convert_range_to_lsp(node.range()),
            severity: Some(DiagnosticSeverity::ERROR),
            code: None,
            code_description: None,
            source: None,
            message: node.to_sexp(),
            related_information: None,
            tags: None,
            data: None,
        })
    }
}

fn tree_errors(tree: &tree_sitter::Tree) -> impl Iterator<Item = tree_sitter::Node<'_>> {
    let mut cursor = tree.root_node().walk();
    (0..tree.root_node().descendant_count()).filter_map(move |i| {
        cursor.goto_descendant(i);
        if cursor.node().is_error() && cursor.node().child_count() > 0 {
            Some(cursor.node())
        } else {
            None
        }
    })
}

impl Server {
    pub fn new(comms: Comms) -> Self {
        Self {
            comms,
            text_documents: HashMap::new(),
        }
    }

    pub fn run(mut self) -> ExitCode {
        let initialize = self
            .comms
            .receive_message::<json_rpc::Request<lsp_types::InitializeParams>>();
        self.comms
            .send_message(&initialize.ok_response(lsp_types::InitializeResult {
                capabilities: lsp_types::ServerCapabilities {
                    hover_provider: Some(HoverProviderCapability::Simple(true)),
                    text_document_sync: Some(lsp_types::TextDocumentSyncCapability::Kind(
                        TextDocumentSyncKind::FULL,
                    )),
                    diagnostic_provider: Some(lsp_types::DiagnosticServerCapabilities::Options(
                        lsp_types::DiagnosticOptions {
                            identifier: None,
                            inter_file_dependencies: false,
                            workspace_diagnostics: false,
                            work_done_progress_options: lsp_types::WorkDoneProgressOptions {
                                work_done_progress: None,
                            },
                        },
                    )),
                    ..Default::default()
                },
                server_info: None,
            }));
        self.comms
            .send_message(&json_rpc::Notification::<()>::new("initialized", None));

        loop {
            let message = self
                .comms
                .receive_message::<json_rpc::Request<serde_json::Value>>();
            match message.method.as_ref() {
                "initialized" => {}
                "shutdown" => {
                    self.comms.send_message(&message.ok_response(()));
                }
                "textDocument/didOpen" => {
                    let message = message
                        .cast_params::<DidOpenTextDocumentParams>()
                        .unwrap()
                        .assert_notification();
                    self.handle_did_open(message);
                }
                "textDocument/didChange" => {
                    let message = message
                        .cast_params::<DidChangeTextDocumentParams>()
                        .unwrap()
                        .assert_notification();
                    self.handle_did_change(message);
                }
                "textDocument/didSave" => {
                    message
                        .cast_params::<DidSaveTextDocumentParams>()
                        .unwrap()
                        .assert_notification();
                }
                "textDocument/hover" => {
                    let message = message.cast_params::<HoverParams>().unwrap();
                    self.handle_hover(message);
                }
                "exit" => return ExitCode::SUCCESS,
                _ => {
                    dbg!(&message);
                    if !message.id.is_null() {
                        self.comms
                            .send_message(&message.error_response(json_rpc::ResponseError {
                                code: json_rpc::METHOD_NOT_FOUND,
                                message: String::from("unknown method"),
                                data: None,
                            }));
                    }
                }
            }
        }
    }

    fn handle_did_open(&mut self, message: Notification<DidOpenTextDocumentParams>) {
        let params = message.params.unwrap();
        let src = params.text_document.text;
        let tree = rain_lang::ast::parser::parse_module_tree_sitter(&src);
        let text_document = TextDocument {
            uri: params.text_document.uri,
            version: (params.text_document.version),
            source: src,
            tree,
        };
        self.comms
            .send_message(&text_document.publish_diagnostics());
        self.text_documents
            .insert(text_document.uri.to_string(), text_document);
    }

    fn handle_did_change(&mut self, message: Notification<DidChangeTextDocumentParams>) {
        let mut params = message.params.unwrap();
        let text_document = self
            .text_documents
            .get_mut(&params.text_document.uri.to_string())
            .unwrap();
        let change = params.content_changes.pop().unwrap();
        let tree = rain_lang::ast::parser::parse_module_tree_sitter(&change.text);
        *text_document = TextDocument {
            uri: params.text_document.uri,
            version: params.text_document.version,
            source: change.text,
            tree,
        };
        self.comms
            .send_message(&text_document.publish_diagnostics());
    }

    fn handle_hover(&mut self, message: Request<HoverParams>) {
        let params = message.params.clone().unwrap();
        let entry = self
            .text_documents
            .get(
                &params
                    .text_document_position_params
                    .text_document
                    .uri
                    .to_string(),
            )
            .unwrap();
        let point = convert_position_to_ts(params.text_document_position_params.position);
        let Some(node) = entry
            .tree
            .root_node()
            .named_descendant_for_point_range(point, point)
        else {
            self.comms
                .send_message(&message.error_response(json_rpc::ResponseError {
                    code: 400,
                    message: String::from("no named node for this span"),
                    data: None,
                }));
            return;
        };
        // let display = node.to_sexp();
        let display = node
            .utf8_text(&entry.source.as_bytes())
            .unwrap()
            .to_string();

        self.comms.send_message(&message.ok_response(Hover {
            contents: lsp_types::HoverContents::Scalar(lsp_types::MarkedString::String(display)),
            range: Some(convert_range_to_lsp(node.range())),
        }));
    }
}

fn convert_position_to_ts(position: lsp_types::Position) -> tree_sitter::Point {
    tree_sitter::Point {
        row: position.line as usize,
        column: position.character as usize,
    }
}

fn convert_range_to_lsp(range: tree_sitter::Range) -> lsp_types::Range {
    lsp_types::Range {
        start: convert_point_to_lsp(range.start_point),
        end: convert_point_to_lsp(range.end_point),
    }
}

fn convert_point_to_lsp(start_point: tree_sitter::Point) -> lsp_types::Position {
    lsp_types::Position {
        line: start_point.row.try_into().unwrap(),
        character: start_point.column.try_into().unwrap(),
    }
}
