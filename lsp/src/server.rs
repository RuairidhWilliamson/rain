use std::{collections::HashMap, process::ExitCode};

use lsp_types::{
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams, HoverProviderCapability,
    TextDocumentSyncKind,
};

use crate::{
    analysis::TextDocument,
    comms::Comms,
    json_rpc::{self, Notification, Request},
};

pub struct Server {
    comms: Comms,
    text_documents: HashMap<String, TextDocument>,
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
                    definition_provider: Some(lsp_types::OneOf::Left(true)),
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
                "textDocument/definition" => {
                    let message = message.cast_params::<GotoDefinitionParams>().unwrap();
                    self.handle_goto_definition(message);
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
        let text_document = TextDocument::from(params.text_document);
        self.comms
            .send_message(&text_document.publish_diagnostics());
        self.text_documents
            .insert(text_document.uri.to_string(), text_document);
    }

    fn handle_did_change(&mut self, message: Notification<DidChangeTextDocumentParams>) {
        let params = message.params.unwrap();
        let text_document = self
            .text_documents
            .get_mut(&params.text_document.uri.to_string())
            .unwrap();
        text_document.change(params);
        self.comms
            .send_message(&text_document.publish_diagnostics());
    }

    fn handle_hover(&mut self, message: Request<HoverParams>) {
        let params = message.params.clone().unwrap();
        let text_document = self
            .text_documents
            .get(
                &params
                    .text_document_position_params
                    .text_document
                    .uri
                    .to_string(),
            )
            .unwrap();
        let Some((display, range)) =
            text_document.hover(params.text_document_position_params.position)
        else {
            self.comms
                .send_message(&message.error_response(json_rpc::ResponseError {
                    code: 400,
                    message: String::from("no named node for this span"),
                    data: None,
                }));
            return;
        };

        self.comms.send_message(&message.ok_response(Hover {
            contents: lsp_types::HoverContents::Scalar(lsp_types::MarkedString::String(display)),
            range: Some(range),
        }));
    }

    fn handle_goto_definition(&mut self, message: Request<GotoDefinitionParams>) {
        let params = message.params.clone().unwrap();
        let text_document = self
            .text_documents
            .get(
                &params
                    .text_document_position_params
                    .text_document
                    .uri
                    .to_string(),
            )
            .unwrap();
        let Some(location) =
            text_document.goto_definition(params.text_document_position_params.position)
        else {
            self.comms
                .send_message(&message.error_response(json_rpc::ResponseError {
                    code: 400,
                    message: String::from("no named node for this span"),
                    data: None,
                }));
            return;
        };
        self.comms
            .send_message(&message.ok_response(GotoDefinitionResponse::Scalar(location)));
    }
}
