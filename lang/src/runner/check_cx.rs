use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use alias::Alias as _;

use crate::{
    ast::{AlternateCondition, BinaryOperatorKind, Node, NodeId},
    ir::IrModule,
    local_span::ErrorLocalSpan,
};

pub struct CheckCx<'a, 'b> {
    pub module: &'a Arc<IrModule>,
    pub locals: HashSet<&'a str>,
    pub captures: HashSet<&'a str>,
    pub args: HashSet<&'a str>,
    pub declaration_names: Arc<HashMap<&'a str, AtomicUsize>>,
    pub errors: &'b mut Vec<ErrorLocalSpan<CheckError>>,
}

impl CheckCx<'_, '_> {
    pub fn check_module(
        module: &Arc<IrModule>,
        check_unused: bool,
    ) -> Vec<ErrorLocalSpan<CheckError>> {
        let mut declaration_names = HashMap::<&str, AtomicUsize>::new();
        let mut errors = Vec::new();
        for d in module.declarations() {
            for name_span in d.assignment.name_spans() {
                let name = name_span.contents(&module.src);
                let entry = declaration_names.entry(name);
                match entry {
                    Entry::Occupied(_) => {
                        errors.push(name_span.with_error(CheckError::ConflictingDeclaration));
                    }
                    Entry::Vacant(vacant) => {
                        vacant.insert(AtomicUsize::new(0));
                    }
                }
            }
        }
        let mut check_cx = CheckCx {
            module: &module.alias(),
            locals: HashSet::new(),
            captures: HashSet::new(),
            args: HashSet::new(),
            declaration_names: Arc::new(declaration_names),
            errors: &mut errors,
        };
        for d in check_cx.module.declarations() {
            check_cx.check_node(d.assignment.expr);
        }
        if check_unused {
            check_cx.check_unused_declarations();
        }
        errors
    }

    pub fn check_unused_declarations(&mut self) {
        for (name, count) in self.declaration_names.iter() {
            if count.load(Ordering::Relaxed) == 0 && *name != "ci" && *name != "main" {
                let Some(did) = self.module.find_declaration_by_name(name) else {
                    unreachable!()
                };
                if self.module.get_declaration(did).pub_token.is_some() {
                    continue;
                }
                let span = self.module.get_declaration_name_span(did);
                self.errors
                    .push(span.with_error(CheckError::UnusedDeclaration));
            }
        }
    }

    fn check_node<'b, 'c>(&'c mut self, nid: NodeId)
    where
        'c: 'b,
    {
        match self.module.get(nid) {
            Node::Ident(tls) => {
                let ident = tls.0.contents(&self.module.src);
                if self.locals.contains(ident) {
                    return;
                }
                if self.args.contains(ident) {
                    return;
                }
                if self.captures.contains(ident) {
                    return;
                }
                if let Some(c) = self.declaration_names.get(ident) {
                    c.fetch_add(1, Ordering::Relaxed);
                    return;
                }
                self.errors.push(tls.0.with_error(CheckError::UnknownIdent));
            }
            Node::Block(block) => {
                for &nid in &block.statements {
                    self.check_node(nid);
                }
            }
            Node::Closure(closure) => {
                let mut callee_cx = self.callee();
                for a in &closure.args {
                    if let Some(a) = &a.type_spec {
                        callee_cx.check_node(a.type_expr);
                    }
                    callee_cx
                        .args
                        .insert(a.name.contents(&callee_cx.module.src));
                }
                if let Some(return_type) = &closure.return_type {
                    callee_cx.check_node(return_type.type_expr);
                }
                callee_cx.check_node(closure.block);
            }
            Node::IfCondition(condition) => {
                Self::check_node(self, condition.condition);
                Self::check_node(self, condition.then_block);
                match &condition.alternate {
                    Some(
                        AlternateCondition::IfElseCondition(alternate)
                        | AlternateCondition::ElseBlock(alternate),
                    ) => {
                        Self::check_node(self, *alternate);
                    }
                    None => {}
                }
            }
            Node::FnCall(fn_call) => {
                Self::check_node(self, fn_call.callee);
                for &a in &fn_call.args {
                    Self::check_node(self, a);
                }
            }
            Node::Assignment(assignment) => {
                for name in assignment.names(&self.module.src) {
                    self.locals.insert(name);
                }
                Self::check_node(self, assignment.expr);
            }
            Node::BinaryOp(binary_op) => {
                Self::check_node(self, binary_op.left);
                match binary_op.op {
                    BinaryOperatorKind::Dot => {}
                    _ => {
                        Self::check_node(self, binary_op.right);
                    }
                }
            }
            Node::List(list) => {
                for element in &list.elements {
                    Self::check_node(self, element.value);
                }
            }
            Node::Record(record) => {
                for field in &record.fields {
                    Self::check_node(self, field.value);
                }
            }
            Node::Not(not) => Self::check_node(self, not.inner),
            Node::FormatStringLiteral(literal) => {
                for &nid in &literal.nodes {
                    Self::check_node(self, nid);
                }
            }
            Node::RawStringLiteral(_)
            | Node::SimpleLiteral(_)
            | Node::StringLiteral(_)
            | Node::IntegerLiteral(_) => {}
        }
    }

    #[must_use]
    pub fn callee<'b, 'c>(&'c mut self) -> CheckCx<'c, 'b>
    where
        'c: 'b,
    {
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
        CheckCx {
            module: self.module,
            locals: HashSet::new(),
            captures,
            args: HashSet::new(),
            declaration_names: self.declaration_names.alias(),
            errors: self.errors,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CheckError {
    #[error("unknown ident")]
    UnknownIdent,
    #[error("unused declaration")]
    UnusedDeclaration,
    #[error("conflicting declaration")]
    ConflictingDeclaration,
}
