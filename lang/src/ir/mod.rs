use std::{borrow::Cow, sync::Arc};

use crate::{
    afs::file::File,
    ast::{ArgTypeSpec, Declare, Module, ModuleRoot, Node, NodeId, error::ParseError},
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
    /// Only None for the embed module
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

    // Get the file of the module, will return [`RunnerError::EmbedContext`] for the embed module
    pub fn file(&self) -> Result<&File, RunnerError> {
        self.file.as_ref().ok_or_else(|| RunnerError::EmbedContext)
    }

    pub fn get(&self, id: NodeId) -> &Node {
        self.inner().0.get(id)
    }

    pub fn span(&self, id: NodeId) -> LocalSpan {
        self.inner().0.span(id)
    }

    pub fn get_declaration(&self, id: LocalDeclarationId) -> &Declare {
        let Some(d) = self.inner().module_root().declarations.get(id.0) else {
            unreachable!()
        };
        d
    }

    pub fn get_declaration_type_spec(&self, id: LocalDeclarationId) -> &Option<ArgTypeSpec> {
        match self.inner().module_root().declarations.get(id.0) {
            Some(let_declare) => match let_declare.type_specs().nth(id.1) {
                Some(type_spec) => type_spec,
                None => unreachable!(),
            },
            None => unreachable!(),
        }
    }

    pub fn get_declaration_name_span(&self, id: LocalDeclarationId) -> LocalSpan {
        match self.inner().module_root().declarations.get(id.0) {
            Some(let_declare) => match let_declare.name_spans().nth(id.1) {
                Some(span) => span,
                None => unreachable!(),
            },
            None => unreachable!(),
        }
    }

    pub fn find_declaration_by_name(&self, name: &str) -> Option<LocalDeclarationId> {
        self.inner()
            .declarations()
            .enumerate()
            .find_map(|(id, let_declare)| {
                let index = let_declare
                    .names(&self.src)
                    .position(|declare_name| declare_name == name)?;
                Some(LocalDeclarationId(id, index))
            })
    }

    pub fn list_declaration_names(&self) -> impl Iterator<Item = &str> {
        self.inner()
            .declarations()
            .flat_map(|let_declare| let_declare.names(&self.src))
    }

    pub fn list_pub_declaration_names(&self) -> impl Iterator<Item = &str> {
        self.inner()
            .declarations()
            .filter(|let_declare| let_declare.pub_token.is_some())
            .flat_map(|let_declare| let_declare.names(&self.src))
    }
}

#[derive(Debug)]
struct ParsedIrModule(Module);

impl ParsedIrModule {
    fn module_root(&self) -> &ModuleRoot {
        &self.0.root
    }

    fn declarations(&self) -> impl Iterator<Item = &Declare> {
        self.module_root().declarations.iter()
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
pub struct LocalDeclarationId(usize, usize);

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
