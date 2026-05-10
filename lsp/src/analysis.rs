use std::{num::TryFromIntError, path::Path, sync::Arc};

use alias::Alias as _;
use lsp_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, Location, Position,
    PublishDiagnosticsParams, Range, TextDocumentItem,
};
use rain_core::{config::Config, driver::DriverImpl};
use rain_lang::{
    afs::{
        File,
        absolute::AbsolutePathBuf,
        local::{entry::LocalFSEntry, file::LocalFile},
        path::SealedFilePath,
    },
    ast::Module,
    ir::{IrModule, Rir},
    local_span::{ErrorLocalSpan, LocalSpan},
    runner::checker::CheckError,
};

use crate::json_rpc::Notification;

pub struct TextDocument {
    pub uri: lsp_types::Uri,
    pub version: i32,
    pub source: String,
    pub tree: tree_sitter::Tree,
}

impl From<TextDocumentItem> for TextDocument {
    fn from(text_document: TextDocumentItem) -> Self {
        Self {
            uri: text_document.uri,
            version: text_document.version,
            tree: rain_lang::ast::ts_parser::parse(&text_document.text),
            source: text_document.text,
        }
    }
}

impl TextDocument {
    pub fn change(&mut self, params: DidChangeTextDocumentParams) {
        assert_eq!(self.uri, params.text_document.uri);
        for change in params.content_changes {
            if let Some(rng) = change.range {
                todo!("implement partial document changes: {rng:?}")
            } else {
                self.source = change.text;
            }
        }
        self.tree = rain_lang::ast::ts_parser::parse(&self.source);
        self.version = params.text_document.version;
    }

    pub fn publish_diagnostics(&self) -> Notification<PublishDiagnosticsParams> {
        Notification::new(
            "textDocument/publishDiagnostics",
            Some(PublishDiagnosticsParams {
                uri: self.uri.clone(),
                version: Some(self.version),
                diagnostics: self.diagnostics().collect(),
            }),
        )
    }

    pub fn hover(&self, position: Position) -> Option<(String, Range)> {
        let span: LocalSpan = LocalSpan::byte_from_line_colz(
            &self.source,
            position.line.try_into().unwrap(),
            position.character.try_into().unwrap(),
        )?;
        let module = self.prepare_module()?;
        let node = module.find_node_by_span(span)?;
        let span = module.span(node);
        let contents = span.contents(&module.src);
        let checker = rain_lang::runner::checker::CheckModuleResult::check_module(&module, true)
            .check_node_type(node);
        Some((
            format!("{contents}\n{checker:?}"),
            convert_span_to_lsp(module.span(node), &self.source).unwrap(),
        ))
    }

    pub fn goto_definition(&self, position: Position) -> Option<Location> {
        let span: LocalSpan = LocalSpan::byte_from_line_colz(
            &self.source,
            position.line.try_into().unwrap(),
            position.character.try_into().unwrap(),
        )?;
        let module = self.prepare_module()?;
        let node = module.find_node_by_span(span)?;
        let span = module.span(node);
        let contents = span.contents(&module.src);
        let id = module.find_declaration_by_name(contents)?;
        let span = module.span(module.get_declaration(id).assignment);
        Some(Location {
            uri: self.uri.clone(),
            range: convert_span_to_lsp(span, &module.src).unwrap(),
        })
    }

    fn diagnostics(&self) -> impl Iterator<Item = Diagnostic> {
        tree_errors(&self.tree)
            .map(|node| Diagnostic {
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
            .chain(self.check_errors().into_iter().map(|err| Diagnostic {
                range: convert_span_to_lsp(err.span, &self.source).unwrap(),
                severity: Some(match err.err {
                    CheckError::UnusedDeclaration => DiagnosticSeverity::WARNING,
                    _ => DiagnosticSeverity::ERROR,
                }),
                code: None,
                code_description: None,
                source: None,
                message: err.err.to_string(),
                related_information: None,
                tags: None,
                data: None,
            }))
    }

    fn check_errors(&self) -> Vec<ErrorLocalSpan<CheckError>> {
        let Some(module) = self.prepare_module() else {
            return Vec::new();
        };
        rain_lang::runner::checker::CheckModuleResult::check_module(&module, true).errors
    }

    fn prepare_module(&self) -> Option<Arc<IrModule>> {
        let config = Config::new();
        let mut ir = Rir::new();
        let driver = DriverImpl::new(config);
        let root = rain_core::find_main_rain().unwrap();
        let area = root.parent().unwrap();
        let path = Path::new(self.uri.path().as_str());
        let rel_path = path.strip_prefix(area).unwrap();
        let rel_path = SealedFilePath::new(rel_path.to_str().unwrap()).unwrap();
        let file = File::Local(
            LocalFile::new_checked(
                &driver,
                LocalFSEntry {
                    area: AbsolutePathBuf(area.to_path_buf()),
                    path: rel_path,
                },
            )
            .unwrap(),
        );
        let src = self.source.clone();
        let module = Module::parse(&src);
        let Ok(mid) = ir.insert_module(Some(file), src, module) else {
            return None;
        };
        let module = ir.get_module(mid);
        Some(module.alias())
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

pub fn convert_span_to_lsp(
    span: LocalSpan,
    src: &str,
) -> Result<lsp_types::Range, TryFromIntError> {
    let start = span.start_line_colz(src);
    let end = span.end_line_colz(src);
    Ok(lsp_types::Range {
        start: Position {
            line: start.0.try_into()?,
            character: start.1.try_into()?,
        },
        end: Position {
            line: end.0.try_into()?,
            character: end.1.try_into()?,
        },
    })
}

pub fn convert_range_to_lsp(range: tree_sitter::Range) -> lsp_types::Range {
    lsp_types::Range {
        start: convert_point_to_lsp(range.start_point),
        end: convert_point_to_lsp(range.end_point),
    }
}

pub fn convert_point_to_lsp(start_point: tree_sitter::Point) -> lsp_types::Position {
    lsp_types::Position {
        line: start_point.row.try_into().unwrap(),
        character: start_point.column.try_into().unwrap(),
    }
}
