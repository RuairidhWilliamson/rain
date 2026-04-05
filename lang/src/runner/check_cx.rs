use std::{
    collections::{HashMap, hash_map::Entry},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use alias::Alias as _;

use crate::{
    ast::{
        AlternateCondition, BinaryOp, BinaryOperatorKind, DeclareName, Node, NodeId, SimpleLiteral,
        SimpleLiteralKind,
    },
    ir::IrModule,
    local_span::ErrorLocalSpan,
    runner::{internal::InternalFunction, value::RainTypeId},
};

pub fn check_module(module: &Arc<IrModule>, check_unused: bool) -> Vec<ErrorLocalSpan<CheckError>> {
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
        previous: None,
        locals: HashMap::new(),
        captures: HashMap::new(),
        args: HashMap::new(),
        declaration_names: Arc::new(declaration_names),
        errors: &mut errors,
    };
    for d in check_cx.module.declarations() {
        check_cx.check_node(d.assignment.expr, CheckValue::Unknown);
    }
    if check_unused {
        check_cx.check_unused_declarations();
    }
    errors
}

#[derive(Clone, Copy)]
enum CheckValue {
    Unknown,
    ExactType(RainTypeId),
}

struct CheckCx<'a, 'b> {
    module: &'a Arc<IrModule>,
    previous: Option<CheckValue>,
    locals: HashMap<&'a str, CheckValue>,
    captures: HashMap<&'a str, CheckValue>,
    args: HashMap<&'a str, CheckValue>,
    declaration_names: Arc<HashMap<&'a str, AtomicUsize>>,
    errors: &'b mut Vec<ErrorLocalSpan<CheckError>>,
}

impl CheckCx<'_, '_> {
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

    fn check_node<'b, 'c>(&'c mut self, nid: NodeId, expected: CheckValue) -> CheckValue
    where
        'c: 'b,
    {
        let actual = self.check_node_inner(nid, expected);
        if let (CheckValue::ExactType(expected), CheckValue::ExactType(actual)) =
            (&expected, &actual)
            && expected != actual
        {
            self.errors.push(
                self.module
                    .span(nid)
                    .with_error(CheckError::TypeError(*expected, *actual)),
            );
        }
        actual
    }

    #[expect(clippy::too_many_lines)]
    fn check_node_inner<'b, 'c>(&'c mut self, nid: NodeId, expected: CheckValue) -> CheckValue
    where
        'c: 'b,
    {
        match self.module.get(nid) {
            Node::Ident(tls) => {
                let ident = tls.0.contents(&self.module.src);
                if let Some(v) = self.locals.get(ident) {
                    return *v;
                }
                if let Some(v) = self.args.get(ident) {
                    return *v;
                }
                if let Some(v) = self.captures.get(ident) {
                    return *v;
                }
                if let Some(c) = self.declaration_names.get(ident) {
                    c.fetch_add(1, Ordering::Relaxed);
                    return CheckValue::Unknown;
                }
                self.errors.push(tls.0.with_error(CheckError::UnknownIdent));
            }
            Node::Block(block) => {
                let [statements @ .., last] = &block.statements[..] else {
                    return CheckValue::ExactType(RainTypeId::Unit);
                };
                for &nid in statements {
                    self.previous = Some(self.check_node(nid, CheckValue::Unknown));
                }
                return self.check_node(*last, expected);
            }
            Node::Closure(closure) => {
                let mut callee_cx = self.callee();
                for a in &closure.args {
                    if let Some(a) = &a.type_spec {
                        callee_cx.check_node(a.type_expr, CheckValue::Unknown);
                    }
                    callee_cx
                        .args
                        .insert(a.name.contents(&callee_cx.module.src), CheckValue::Unknown);
                }
                if let Some(return_type) = &closure.return_type {
                    callee_cx.check_node(return_type.type_expr, CheckValue::Unknown);
                }
                callee_cx.check_node(closure.block, CheckValue::Unknown);
            }
            Node::IfCondition(condition) => {
                Self::check_node(
                    self,
                    condition.condition,
                    CheckValue::ExactType(RainTypeId::Boolean),
                );
                Self::check_node(self, condition.then_block, expected);
                match &condition.alternate {
                    Some(
                        AlternateCondition::IfElseCondition(alternate)
                        | AlternateCondition::ElseBlock(alternate),
                    ) => {
                        Self::check_node(self, *alternate, expected);
                    }
                    None => {}
                }
            }
            Node::FnCall(fn_call) => {
                Self::check_node(self, fn_call.callee, CheckValue::Unknown);
                for &a in &fn_call.args {
                    Self::check_node(self, a, CheckValue::Unknown);
                }
            }
            Node::Assignment(assignment) => {
                match &assignment.name {
                    DeclareName::Single(declare) => {
                        let rhs = Self::check_node(self, assignment.expr, CheckValue::Unknown);
                        let name = declare.name.contents(&self.module.src);
                        if let Some(type_spec) = &declare.type_spec {
                            self.check_node(type_spec.type_expr, CheckValue::Unknown);
                        }
                        self.locals.insert(name, rhs);
                    }
                    DeclareName::NamedDestructure(declare) => {
                        Self::check_node(self, assignment.expr, CheckValue::Unknown);
                        for e in &declare.elements {
                            let name = e.name.contents(&self.module.src);
                            if let Some(type_spec) = &e.type_spec {
                                self.check_node(type_spec.type_expr, CheckValue::Unknown);
                            }
                            self.locals.insert(name, CheckValue::Unknown);
                        }
                    }
                    DeclareName::SequenceDestructure(declare) => {
                        Self::check_node(
                            self,
                            assignment.expr,
                            CheckValue::ExactType(RainTypeId::List),
                        );
                        for e in &declare.elements {
                            let name = e.name.contents(&self.module.src);
                            if let Some(type_spec) = &e.type_spec {
                                self.check_node(type_spec.type_expr, CheckValue::Unknown);
                            }
                            self.locals.insert(name, CheckValue::Unknown);
                        }
                    }
                }
                return CheckValue::Unknown;
            }
            Node::BinaryOp(BinaryOp {
                left,
                op: BinaryOperatorKind::Dot,
                right,
                ..
            }) => {
                if matches!(
                    Self::check_node(self, *left, CheckValue::Unknown),
                    CheckValue::ExactType(RainTypeId::Internal)
                ) && let Node::Ident(tls) = self.module.get(*right)
                {
                    let name = tls.0.contents(&self.module.src);
                    if InternalFunction::evaluate_internal_function_name(name).is_none() {
                        self.errors
                            .push(tls.0.with_error(CheckError::InvalidInternal));
                    } else {
                        return CheckValue::ExactType(RainTypeId::InternalFunction);
                    }
                }
            }
            Node::BinaryOp(binary_op) => {
                Self::check_node(self, binary_op.left, CheckValue::Unknown);
                Self::check_node(self, binary_op.right, CheckValue::Unknown);
            }
            Node::List(list) => {
                for element in &list.elements {
                    Self::check_node(self, element.value, CheckValue::Unknown);
                }
                return CheckValue::ExactType(RainTypeId::List);
            }
            Node::Record(record) => {
                for field in &record.fields {
                    Self::check_node(self, field.value, CheckValue::Unknown);
                }
                return CheckValue::ExactType(RainTypeId::Record);
            }
            Node::Not(not) => {
                Self::check_node(self, not.inner, CheckValue::ExactType(RainTypeId::Boolean));
                return CheckValue::ExactType(RainTypeId::Boolean);
            }
            Node::FormatStringLiteral(literal) => {
                for &nid in &literal.nodes {
                    Self::check_node(self, nid, CheckValue::Unknown);
                }
                return CheckValue::ExactType(RainTypeId::String);
            }
            Node::SimpleLiteral(SimpleLiteral {
                kind: SimpleLiteralKind::Internal,
                ..
            }) => return CheckValue::ExactType(RainTypeId::Internal),
            Node::SimpleLiteral(SimpleLiteral {
                kind: SimpleLiteralKind::True | SimpleLiteralKind::False,
                ..
            }) => return CheckValue::ExactType(RainTypeId::Boolean),
            Node::SimpleLiteral(SimpleLiteral {
                kind:
                    SimpleLiteralKind::Import | SimpleLiteralKind::Stdlib | SimpleLiteralKind::ThisFile,
                ..
            }) => return CheckValue::ExactType(RainTypeId::Closure),
            Node::SimpleLiteral(SimpleLiteral {
                kind: SimpleLiteralKind::Underscore,
                span,
            }) => {
                if let Some(prev) = self.previous {
                    return prev;
                }
                self.errors
                    .push(span.with_error(CheckError::InvalidUnderscore));
                return CheckValue::Unknown;
            }
            Node::IntegerLiteral(_) => return CheckValue::ExactType(RainTypeId::Integer),
            Node::RawStringLiteral(_) | Node::StringLiteral(_) => {
                return CheckValue::ExactType(RainTypeId::String);
            }
        }
        CheckValue::Unknown
    }

    #[must_use]
    pub fn callee<'b, 'c>(&'c mut self) -> CheckCx<'c, 'b>
    where
        'c: 'b,
    {
        let mut captures = self.captures.clone();
        for (&name, &v) in &self.args {
            captures.insert(name, v);
        }
        for (&name, &v) in &self.locals {
            captures.insert(name, v);
        }
        CheckCx {
            module: self.module,
            previous: None,
            locals: HashMap::new(),
            captures,
            args: HashMap::new(),
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
    #[error("invalid internal")]
    InvalidInternal,
    #[error("type error: expected {0}, actual {1}")]
    TypeError(RainTypeId, RainTypeId),
    #[error("invalid underscore, no previous statement")]
    InvalidUnderscore,
}
