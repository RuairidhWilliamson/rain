use std::path::PathBuf;

use crate::ast2::{Module, ModuleRoot, Node, NodeId};

#[derive(Debug, Default)]
pub struct Rir {
    modules: Vec<IrModule>,
}

impl Rir {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_module(&mut self, path: Option<PathBuf>, src: String, ast: Module) -> ModuleId {
        let id = ModuleId(self.modules.len());
        self.modules.push(IrModule {
            id,
            path,
            src,
            module: ast,
        });
        id
    }

    pub fn get_module(&self, module_id: ModuleId) -> &IrModule {
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
        self.get_module(module_id)
            .find_declaration_by_name(name)
            .map(|id| DeclarationId(module_id, id))
    }
}

#[derive(Debug)]
pub struct IrModule {
    pub id: ModuleId,
    #[allow(dead_code)]
    path: Option<PathBuf>,
    pub src: String,
    pub module: crate::ast2::Module,
}

impl IrModule {
    pub fn get_declaration(&self, id: LocalDeclarationId) -> NodeId {
        let Node::ModuleRoot(module_root) = self.module.get(self.module.root) else {
            unreachable!()
        };
        let Some(id) = module_root.declarations.get(id.0) else {
            unreachable!()
        };
        *id
    }

    fn module_root(&self) -> &ModuleRoot {
        let Node::ModuleRoot(module_root) = self.module.get(self.module.root) else {
            unreachable!()
        };
        module_root
    }

    fn declarations(&self) -> impl Iterator<Item = &Node> {
        self.module_root()
            .declarations
            .iter()
            .map(|nid| self.module.get(*nid))
    }

    fn find_declaration_by_name(&self, name: &str) -> Option<LocalDeclarationId> {
        self.declarations()
            .enumerate()
            .find(|(_, node)| match node {
                Node::LetDeclare(let_declare) => let_declare.name.span.contents(&self.src) == name,
                Node::FnDeclare(fn_declare) => fn_declare.name.span.contents(&self.src) == name,
                _ => unreachable!(),
            })
            .map(|(id, _)| LocalDeclarationId(id))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct ModuleId(usize);

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct LocalDeclarationId(usize);

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

#[derive(Debug)]
pub enum RainError {
    UnresolvedIdentifier,
}

impl std::fmt::Display for RainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnresolvedIdentifier => f.write_str("unresolved identifier"),
        }
    }
}

impl std::error::Error for RainError {}
