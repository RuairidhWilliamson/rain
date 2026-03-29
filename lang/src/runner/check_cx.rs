use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, atomic::AtomicUsize},
};

use alias::Alias as _;

use crate::ir::IrModule;

pub struct CheckCx<'a> {
    pub module: &'a Arc<IrModule>,
    pub locals: HashSet<&'a str>,
    pub captures: HashSet<&'a str>,
    pub args: HashSet<&'a str>,
    pub declaration_names: Arc<HashMap<&'a str, AtomicUsize>>,
}

impl CheckCx<'_> {
    #[must_use]
    pub fn callee(&self) -> Self {
        let mut captures = HashSet::new();
        for &name in &self.locals {
            captures.insert(name);
        }
        for &name in &self.captures {
            captures.insert(name);
        }
        for &name in &self.args {
            captures.insert(name);
        }
        Self {
            module: self.module,
            locals: HashSet::new(),
            captures,
            args: HashSet::new(),
            declaration_names: self.declaration_names.alias(),
        }
    }
}
