use std::{borrow::Cow, sync::Arc};

use crate::{
    afs::file::File,
    ast::{Module, ModuleRoot, Node, NodeId, error::ParseError},
    local_span::{ErrorLocalSpan, LocalSpan},
    runner::error::RunnerError,
    span::ErrorSpan,
};

#[derive(Debug, Default)]
pub struct Rir {
    modules: Vec<Arc<IrModule>>,
}

impl Rir {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_module(
        &mut self,
        file: Option<File>,
        src: impl Into<Cow<'static, str>>,
        ast: Result<Module, ErrorLocalSpan<ParseError>>,
    ) -> Result<ModuleId, ErrorSpan<ParseError>> {
        let id = ModuleId(self.modules.len());
        let (module, res) = match ast {
            Ok(m) => (Some(ParsedIrModule(m)), Ok(id)),
            Err(els) => (None, Err(els.upgrade(id))),
        };
        self.modules.push(Arc::new(IrModule {
            id,
            file,
            src: src.into(),
            module,
        }));
        res
    }

    pub fn get_module(&self, module_id: ModuleId) -> &Arc<IrModule> {
        let Some(m) = self.modules.get(module_id.0) else {
            unreachable!("id is always valid")
        };
        m
    }

    pub fn resolve_global_declaration(
        &self,
        module_id: ModuleId,
        name: &str,
    ) -> Option<DeclarationId> {
        let module = self.get_module(module_id);
        module
            .find_declaration_by_name(name)
            .map(|id| DeclarationId(module_id, id))
    }

    pub fn len(&self) -> usize {
        self.modules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }
}

#[derive(Debug)]
pub struct IrModule {
    pub id: ModuleId,
    /// Only None for the prelude module
    pub file: Option<File>,
    pub src: Cow<'static, str>,
    module: Option<ParsedIrModule>,
}

impl IrModule {
    fn inner(&self) -> &ParsedIrModule {
        let Some(m) = &self.module else {
            unreachable!("module failed to parse so can't be used")
        };
        m
    }

    // Get the file of the module, will return [`RunnerError::PreludeContext`] for the prelude module
    pub fn file(&self) -> Result<&File, RunnerError> {
        self.file
            .as_ref()
            .ok_or_else(|| RunnerError::PreludeContext)
    }

    pub fn get(&self, id: NodeId) -> &Node {
        self.inner().0.get(id)
    }

    pub fn span(&self, id: NodeId) -> LocalSpan {
        self.inner().0.span(id)
    }

    pub fn get_declaration(&self, id: LocalDeclarationId) -> NodeId {
        let Some(id) = self.inner().module_root().declarations.get(id.0) else {
            unreachable!()
        };
        *id
    }

    pub fn get_declaration_name_span(&self, id: LocalDeclarationId) -> LocalSpan {
        match self.get(self.get_declaration(id)) {
            Node::LetDeclare(let_declare) => let_declare.name.span,
            Node::FnDeclare(fn_declare) => fn_declare.name.span,
            _ => unreachable!(),
        }
    }

    pub fn find_declaration_by_name(&self, name: &str) -> Option<LocalDeclarationId> {
        self.inner()
            .declarations()
            .enumerate()
            .find(|(_, node)| match node {
                Node::LetDeclare(let_declare) => let_declare.name.span.contents(&self.src) == name,
                Node::FnDeclare(fn_declare) => fn_declare.name.span.contents(&self.src) == name,
                _ => unreachable!(),
            })
            .map(|(id, _)| LocalDeclarationId(id))
    }

    pub fn list_fn_declaration_names(&self) -> impl Iterator<Item = &str> {
        self.inner().declarations().filter_map(|node| match node {
            Node::FnDeclare(fn_declare) => Some(fn_declare.name.span.contents(&self.src)),
            Node::LetDeclare(_) => None,
            _ => unreachable!(),
        })
    }

    pub fn list_pub_fn_declaration_names(&self) -> impl Iterator<Item = &str> {
        self.inner().declarations().filter_map(|node| match node {
            Node::FnDeclare(fn_declare) if fn_declare.pub_token.is_some() => {
                Some(fn_declare.name.span.contents(&self.src))
            }
            Node::LetDeclare(_) | Node::FnDeclare(_) => None,
            _ => unreachable!(),
        })
    }
}

#[derive(Debug)]
struct ParsedIrModule(Module);

impl ParsedIrModule {
    fn module_root(&self) -> &ModuleRoot {
        let Node::ModuleRoot(module_root) = self.0.get(self.0.root) else {
            unreachable!()
        };
        module_root
    }

    fn declarations(&self) -> impl Iterator<Item = &Node> {
        self.module_root()
            .declarations
            .iter()
            .map(|nid| self.0.get(*nid))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct ModuleId(usize);

impl std::fmt::Display for ModuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("Module<{}>", self.0))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct LocalDeclarationId(usize);

impl std::fmt::Display for LocalDeclarationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("LocalDeclaration<{}>", self.0))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct DeclarationId(ModuleId, LocalDeclarationId);

impl DeclarationId {
    pub fn module_id(&self) -> ModuleId {
        self.0
    }

    pub fn local_id(&self) -> LocalDeclarationId {
        self.1
    }
}

impl std::fmt::Display for DeclarationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "Declaration<{}, {}>",
            self.module_id().0,
            self.local_id().0,
        ))
    }
}
