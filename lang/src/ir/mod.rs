use std::path::Path;

pub struct IR<'a> {
    pub modules: Vec<Module<'a>>,
}

impl IR<'_> {
    pub fn declaration_deps(&self, id: DeclarationId) -> Vec<DeclarationId> {
        todo!("{:?}", id)
    }
}

pub struct Module<'a> {
    pub path: &'a Path,
    pub src: &'a str,
    pub ast: &'a crate::ast::Script,
    pub declaration: Vec<&'a crate::ast::Declaration>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct ModuleId(usize);

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct DeclarationId(ModuleId, usize);
