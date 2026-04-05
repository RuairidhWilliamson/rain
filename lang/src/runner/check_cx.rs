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
    runner::{
        internal::InternalFunction,
        value::{RainTypeId, Value},
    },
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

pub fn check_node_type(module: &Arc<IrModule>, node: NodeId) -> CheckValue {
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
    check_cx.check_node(node, CheckValue::Unknown)
}

#[derive(Debug, Clone)]
pub enum CheckValue {
    /// No clue, could be anything
    Unknown,
    /// The value is known to be of this type
    ExactType(RainTypeId),
    ExactValue(Value),
}

impl CheckValue {
    const fn exact_type(&self) -> Option<RainTypeId> {
        match self {
            Self::Unknown => None,
            Self::ExactType(rain_type_id) => Some(*rain_type_id),
            Self::ExactValue(value) => Some(value.rain_type_id()),
        }
    }
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
    fn check_unused_declarations(&mut self) {
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
        let actual = self.check_node_inner(nid, expected.clone());
        // Check the type
        match (&expected, actual.exact_type()) {
            (CheckValue::ExactType(expected), Some(actual)) if *expected != actual => {
                self.errors.push(
                    self.module
                        .span(nid)
                        .with_error(CheckError::TypeError(*expected, actual)),
                );
            }
            _ => (),
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
                    return v.clone();
                }
                if let Some(v) = self.args.get(ident) {
                    return v.clone();
                }
                if let Some(v) = self.captures.get(ident) {
                    return v.clone();
                }
                if let Some(c) = self.declaration_names.get(ident) {
                    c.fetch_add(1, Ordering::Relaxed);
                    return CheckValue::Unknown;
                }
                self.errors.push(tls.0.with_error(CheckError::UnknownIdent));
                CheckValue::Unknown
            }
            Node::Block(block) => {
                let [statements @ .., last] = &block.statements[..] else {
                    return CheckValue::ExactType(RainTypeId::Unit);
                };
                for &nid in statements {
                    self.previous = Some(self.check_node(nid, CheckValue::Unknown));
                }
                self.check_node(*last, expected)
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
                CheckValue::ExactType(RainTypeId::Closure)
            }
            Node::IfCondition(condition) => {
                Self::check_node(
                    self,
                    condition.condition,
                    CheckValue::ExactType(RainTypeId::Boolean),
                );
                Self::check_node(self, condition.then_block, expected.clone());
                match &condition.alternate {
                    Some(
                        AlternateCondition::IfElseCondition(alternate)
                        | AlternateCondition::ElseBlock(alternate),
                    ) => {
                        Self::check_node(self, *alternate, expected);
                    }
                    None => {}
                }
                CheckValue::Unknown
            }
            Node::FnCall(fn_call) => {
                let callee = Self::check_node(self, fn_call.callee, CheckValue::Unknown);
                match callee {
                    CheckValue::ExactValue(Value::InternalFunction(InternalFunction::GetType)) => {
                        if let &[a] = &fn_call.args[..] {
                            if let Some(exact_type) =
                                Self::check_node(self, a, CheckValue::Unknown).exact_type()
                            {
                                return CheckValue::ExactType(exact_type);
                            }
                        } else {
                            self.errors.push(
                                fn_call
                                    .rparen_token
                                    .with_error(CheckError::WrongArgCount(1, fn_call.args.len())),
                            );
                        }
                    }
                    CheckValue::ExactValue(Value::InternalFunction(InternalFunction::Unit)) => {
                        if let &[] = &fn_call.args[..] {
                            return CheckValue::ExactValue(Value::Unit);
                        } else {
                            self.errors.push(
                                fn_call
                                    .rparen_token
                                    .with_error(CheckError::WrongArgCount(0, fn_call.args.len())),
                            );
                        }
                    }
                    _ => {}
                }
                for &a in &fn_call.args {
                    Self::check_node(self, a, CheckValue::Unknown);
                }
                CheckValue::Unknown
            }
            Node::Assignment(assignment) => {
                match &assignment.name {
                    DeclareName::Single(declare) => {
                        let name = declare.name.contents(&self.module.src);
                        let expected = if let Some(type_spec) = &declare.type_spec {
                            self.check_node(type_spec.type_expr, CheckValue::Unknown)
                        } else {
                            CheckValue::Unknown
                        };
                        let rhs = Self::check_node(self, assignment.expr, expected);
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
                CheckValue::Unknown
            }
            Node::BinaryOp(BinaryOp {
                left,
                op: BinaryOperatorKind::Dot,
                right,
                ..
            }) => {
                if matches!(
                    Self::check_node(self, *left, CheckValue::Unknown).exact_type(),
                    Some(RainTypeId::Internal)
                ) && let Node::Ident(tls) = self.module.get(*right)
                {
                    let name = tls.0.contents(&self.module.src);
                    if let Some(internal_function) =
                        InternalFunction::evaluate_internal_function_name(name)
                    {
                        return CheckValue::ExactValue(Value::InternalFunction(internal_function));
                    }
                    self.errors
                        .push(tls.0.with_error(CheckError::InvalidInternal));
                }
                CheckValue::Unknown
            }
            Node::BinaryOp(BinaryOp {
                left,
                op: BinaryOperatorKind::LogicalAnd | BinaryOperatorKind::LogicalOr,
                right,
                ..
            }) => {
                Self::check_node(self, *left, CheckValue::ExactType(RainTypeId::Boolean));
                Self::check_node(self, *right, CheckValue::ExactType(RainTypeId::Boolean));
                CheckValue::ExactType(RainTypeId::Boolean)
            }
            Node::BinaryOp(BinaryOp {
                left,
                op: BinaryOperatorKind::Equals | BinaryOperatorKind::NotEquals,
                right,
                ..
            }) => {
                Self::check_node(self, *left, CheckValue::Unknown);
                Self::check_node(self, *right, CheckValue::Unknown);
                CheckValue::ExactType(RainTypeId::Boolean)
            }
            Node::BinaryOp(BinaryOp {
                left,
                op:
                    BinaryOperatorKind::Addition
                    | BinaryOperatorKind::Subtraction
                    | BinaryOperatorKind::Multiplication
                    | BinaryOperatorKind::Division
                    | BinaryOperatorKind::Modulo
                    | BinaryOperatorKind::Pow
                    | BinaryOperatorKind::BitwiseAnd
                    | BinaryOperatorKind::BitwiseOr,
                right,
                ..
            }) => {
                // These operators are all of the form fn(T, T) -> T
                let lhs = Self::check_node(self, *left, CheckValue::Unknown);
                let rhs = Self::check_node(
                    self,
                    *right,
                    lhs.exact_type()
                        .map(CheckValue::ExactType)
                        .unwrap_or(CheckValue::Unknown),
                );
                lhs.exact_type()
                    .or(rhs.exact_type())
                    .map(CheckValue::ExactType)
                    .unwrap_or(CheckValue::Unknown)
            }
            Node::BinaryOp(BinaryOp { left, right, .. }) => {
                Self::check_node(self, *left, CheckValue::Unknown);
                Self::check_node(self, *right, CheckValue::Unknown);
                CheckValue::Unknown
            }
            Node::List(list) => {
                for element in &list.elements {
                    Self::check_node(self, element.value, CheckValue::Unknown);
                }
                CheckValue::ExactType(RainTypeId::List)
            }
            Node::Record(record) => {
                for field in &record.fields {
                    Self::check_node(self, field.value, CheckValue::Unknown);
                }
                CheckValue::ExactType(RainTypeId::Record)
            }
            Node::Not(not) => {
                Self::check_node(self, not.inner, CheckValue::ExactType(RainTypeId::Boolean));
                CheckValue::ExactType(RainTypeId::Boolean)
            }
            Node::FormatStringLiteral(literal) => {
                for &nid in &literal.nodes {
                    Self::check_node(self, nid, CheckValue::Unknown);
                }
                CheckValue::ExactType(RainTypeId::String)
            }
            Node::SimpleLiteral(SimpleLiteral {
                kind: SimpleLiteralKind::Internal,
                ..
            }) => CheckValue::ExactValue(Value::Internal),
            Node::SimpleLiteral(SimpleLiteral {
                kind: SimpleLiteralKind::True,
                ..
            }) => CheckValue::ExactValue(Value::Boolean(true)),
            Node::SimpleLiteral(SimpleLiteral {
                kind: SimpleLiteralKind::False,
                ..
            }) => CheckValue::ExactValue(Value::Boolean(false)),
            Node::SimpleLiteral(SimpleLiteral {
                kind:
                    SimpleLiteralKind::Import | SimpleLiteralKind::Stdlib | SimpleLiteralKind::ThisFile,
                ..
            }) => CheckValue::ExactType(RainTypeId::Closure),
            Node::SimpleLiteral(SimpleLiteral {
                kind: SimpleLiteralKind::Underscore,
                span,
            }) => {
                if let Some(prev) = &self.previous {
                    return prev.clone();
                }
                self.errors
                    .push(span.with_error(CheckError::InvalidUnderscore));
                CheckValue::Unknown
            }
            Node::IntegerLiteral(_) => CheckValue::ExactType(RainTypeId::Integer),
            Node::RawStringLiteral(_) | Node::StringLiteral(_) => {
                CheckValue::ExactType(RainTypeId::String)
            }
        }
    }

    #[must_use]
    fn callee<'b, 'c>(&'c mut self) -> CheckCx<'c, 'b>
    where
        'c: 'b,
    {
        let mut captures = self.captures.clone();
        for (&name, v) in &self.args {
            captures.insert(name, v.clone());
        }
        for (&name, v) in &self.locals {
            captures.insert(name, v.clone());
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
    #[error("wrong number of arguments: expected {0}, actual {1}")]
    WrongArgCount(usize, usize),
}
