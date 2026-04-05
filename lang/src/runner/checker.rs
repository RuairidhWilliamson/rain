use std::{
    collections::{HashMap, hash_map::Entry},
    sync::{Arc, atomic::AtomicUsize},
};

use alias::Alias as _;

use crate::{
    ast::{
        AlternateCondition, BinaryOp, BinaryOperatorKind, DeclareName, Node, NodeId, SimpleLiteral,
        SimpleLiteralKind,
    },
    ir::{IrModule, LocalDeclarationId},
    local_span::ErrorLocalSpan,
    runner::{
        internal::InternalFunction,
        value::{RainTypeId, Value},
    },
};

pub struct CheckModuleResult {
    pub errors: Vec<ErrorLocalSpan<CheckError>>,
    pub node_types: HashMap<NodeId, CheckValue>,
}

impl CheckModuleResult {
    pub fn check_module(module: &Arc<IrModule>, check_unused: bool) -> Self {
        let mut declaration_names = HashMap::<&str, AtomicUsize>::new();
        let mut errors = Vec::new();
        for (_, assignment) in module.declaration_assignments() {
            for name_span in assignment.name_spans() {
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
        let mut node_types = HashMap::new();
        let mut declarations = HashMap::new();
        let mut check_cx = CheckCx {
            module: &module.alias(),
            previous: None,
            locals: None,
            captures: HashMap::new(),
            args: HashMap::new(),
            errors: &mut errors,
            node_types: &mut node_types,
            declarations: &mut declarations,
        };
        for d in check_cx.module.declarations() {
            check_cx.check_node(d.assignment, CheckValue::Unknown);
        }
        if check_unused {
            check_cx.check_unused_declarations();
        }
        Self { errors, node_types }
    }

    pub fn check_node_type(&self, node: NodeId) -> CheckValue {
        self.node_types.get(&node).cloned().unwrap_or_default()
    }
}

#[derive(Debug, Default, Clone)]
pub enum CheckValue {
    /// No clue, could be anything
    #[default]
    Unknown,
    /// The value is known to be of this type but it is not known what value it is within the type
    ExactType(RainTypeId),
    /// The value is known to be this value
    ExactValue(Value),
    FunctionCall {
        args: Vec<Self>,
        return_type: Box<Self>,
    },
}

impl CheckValue {
    fn exact_type(&self) -> Option<RainTypeId> {
        match self {
            Self::Unknown | Self::FunctionCall { .. } => None,
            Self::ExactType(rain_type_id) => Some(*rain_type_id),
            Self::ExactValue(value) => Some(value.rain_type_id()),
        }
    }

    fn type_constraint(&self) -> Option<RainTypeId> {
        match self {
            Self::ExactValue(Value::Type(rain_type_id)) => Some(*rain_type_id),
            _ => None,
        }
    }
}

struct CheckCx<'a, 'b> {
    module: &'a Arc<IrModule>,
    previous: Option<CheckValue>,
    // None when in the global declaration scope
    locals: Option<HashMap<&'a str, CheckValue>>,
    captures: HashMap<&'a str, CheckValue>,
    args: HashMap<&'a str, CheckValue>,
    errors: &'b mut Vec<ErrorLocalSpan<CheckError>>,
    node_types: &'b mut HashMap<NodeId, CheckValue>,
    declarations: &'b mut HashMap<LocalDeclarationId, Declaration>,
}

#[derive(Default)]
struct Declaration {
    value: CheckValue,
    usage: usize,
}

impl CheckCx<'_, '_> {
    fn check_unused_declarations(&mut self) {
        for (&id, Declaration { usage, .. }) in self.declarations.iter() {
            let span = self.module.get_declaration_name_span(id);
            let name = span.contents(&self.module.src);
            if *usage == 0 && name != "ci" && name != "main" {
                if self.module.get_declaration(id).pub_token.is_some() {
                    continue;
                }
                self.errors
                    .push(span.with_error(CheckError::UnusedDeclaration));
            }
        }
    }

    #[expect(clippy::needless_pass_by_value)]
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
        self.node_types.insert(nid, actual.clone());
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
                if let Some(locals) = &self.locals
                    && let Some(v) = locals.get(ident)
                {
                    return v.clone();
                }
                if let Some(v) = self.args.get(ident) {
                    return v.clone();
                }
                if let Some(v) = self.captures.get(ident) {
                    return v.clone();
                }
                if let Some(id) = self.module.find_declaration_by_name(ident) {
                    let declaration = self.declarations.entry(id).or_default();
                    declaration.usage += 1;
                    return declaration.value.clone();
                }
                self.errors.push(tls.0.with_error(CheckError::UnknownIdent));
                CheckValue::Unknown
            }
            Node::Block(block) => {
                let [statements @ .., last] = &block.statements[..] else {
                    return CheckValue::ExactValue(Value::Unit);
                };
                for &nid in statements {
                    self.previous = Some(self.check_node(nid, CheckValue::Unknown));
                }
                self.check_node(*last, expected)
            }
            Node::Closure(closure) => {
                let mut callee_cx = self.callee();
                let mut args = Vec::new();
                for a in &closure.args {
                    let expected = if let Some(a) = &a.type_spec {
                        callee_cx
                            .check_node(a.type_expr, CheckValue::Unknown)
                            .type_constraint()
                            .map(CheckValue::ExactType)
                            .unwrap_or_default()
                    } else {
                        CheckValue::Unknown
                    };
                    args.push(expected.clone());
                    callee_cx
                        .args
                        .insert(a.name.contents(&callee_cx.module.src), expected);
                }
                let expected = if let Some(return_type) = &closure.return_type {
                    callee_cx
                        .check_node(return_type.type_expr, CheckValue::Unknown)
                        .type_constraint()
                        .map(CheckValue::ExactType)
                        .unwrap_or_default()
                } else {
                    CheckValue::Unknown
                };
                callee_cx.check_node(closure.block, expected.clone());
                CheckValue::FunctionCall {
                    args,
                    return_type: Box::new(expected),
                }
            }
            Node::FnCall(fn_call) => {
                let callee = self.check_node(fn_call.callee, CheckValue::Unknown);
                match callee {
                    CheckValue::ExactValue(Value::InternalFunction(InternalFunction::GetType)) => {
                        if let &[a] = &fn_call.args[..] {
                            if let Some(exact_type) =
                                self.check_node(a, CheckValue::Unknown).exact_type()
                            {
                                return CheckValue::ExactValue(Value::Type(exact_type));
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
                        }
                        self.errors.push(
                            fn_call
                                .rparen_token
                                .with_error(CheckError::WrongArgCount(0, fn_call.args.len())),
                        );
                    }
                    CheckValue::FunctionCall { args, return_type } => {
                        if fn_call.args.len() != args.len() {
                            self.errors.push(fn_call.rparen_token.with_error(
                                CheckError::WrongArgCount(args.len(), fn_call.args.len()),
                            ));
                        }
                        for (expected, arg) in args.into_iter().zip(&fn_call.args) {
                            self.check_node(*arg, expected);
                        }
                        return *return_type;
                    }
                    _ => {}
                }
                for &a in &fn_call.args {
                    self.check_node(a, CheckValue::Unknown);
                }
                CheckValue::Unknown
            }
            Node::IfCondition(condition) => {
                Self::check_node(
                    self,
                    condition.condition,
                    CheckValue::ExactType(RainTypeId::Boolean),
                );
                self.check_node(condition.then_block, expected.clone());
                match &condition.alternate {
                    Some(
                        AlternateCondition::IfElseCondition(alternate)
                        | AlternateCondition::ElseBlock(alternate),
                    ) => {
                        self.check_node(*alternate, expected);
                    }
                    None => {}
                }
                CheckValue::Unknown
            }
            Node::Assignment(assignment) => {
                match &assignment.name {
                    DeclareName::Single(declare) => {
                        let name = declare.name.contents(&self.module.src);
                        let expected = if let Some(type_spec) = &declare.type_spec {
                            self.check_node(type_spec.type_expr, CheckValue::Unknown)
                                .type_constraint()
                                .map(CheckValue::ExactType)
                                .unwrap_or(CheckValue::Unknown)
                        } else {
                            CheckValue::Unknown
                        };
                        let rhs = self.check_node(assignment.expr, expected);
                        if let Some(locals) = &mut self.locals {
                            locals.insert(name, rhs);
                        }
                    }
                    DeclareName::NamedDestructure(declare) => {
                        self.check_node(assignment.expr, CheckValue::Unknown);
                        for e in &declare.elements {
                            let name = e.name.contents(&self.module.src);
                            if let Some(type_spec) = &e.type_spec {
                                self.check_node(type_spec.type_expr, CheckValue::Unknown);
                            }
                            if let Some(locals) = &mut self.locals {
                                locals.insert(name, CheckValue::Unknown);
                            }
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
                            if let Some(locals) = &mut self.locals {
                                locals.insert(name, CheckValue::Unknown);
                            }
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
                    self.check_node(*left, CheckValue::Unknown).exact_type(),
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
                self.check_node(*left, CheckValue::ExactType(RainTypeId::Boolean));
                self.check_node(*right, CheckValue::ExactType(RainTypeId::Boolean));
                CheckValue::ExactType(RainTypeId::Boolean)
            }
            Node::BinaryOp(BinaryOp {
                left,
                op: BinaryOperatorKind::Equals | BinaryOperatorKind::NotEquals,
                right,
                ..
            }) => {
                self.check_node(*left, CheckValue::Unknown);
                self.check_node(*right, CheckValue::Unknown);
                CheckValue::ExactType(RainTypeId::Boolean)
            }
            Node::BinaryOp(BinaryOp {
                left,
                op:
                    BinaryOperatorKind::LessThan
                    | BinaryOperatorKind::LessThanEquals
                    | BinaryOperatorKind::GreaterThan
                    | BinaryOperatorKind::GreaterThanEquals,
                right,
                ..
            }) => {
                self.check_node(*left, CheckValue::ExactType(RainTypeId::Integer));
                self.check_node(*right, CheckValue::ExactType(RainTypeId::Integer));
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
                let lhs = self.check_node(*left, CheckValue::Unknown);
                let rhs = self.check_node(
                    *right,
                    lhs.exact_type()
                        .map(CheckValue::ExactType)
                        .unwrap_or_default(),
                );
                lhs.exact_type()
                    .or_else(|| rhs.exact_type())
                    .map(CheckValue::ExactType)
                    .unwrap_or_default()
            }
            Node::List(list) => {
                for element in &list.elements {
                    self.check_node(element.value, CheckValue::Unknown);
                }
                CheckValue::ExactType(RainTypeId::List)
            }
            Node::Record(record) => {
                for field in &record.fields {
                    self.check_node(field.value, CheckValue::Unknown);
                }
                CheckValue::ExactType(RainTypeId::Record)
            }
            Node::Not(not) => {
                match self.check_node(not.inner, CheckValue::ExactType(RainTypeId::Boolean)) {
                    CheckValue::Unknown | CheckValue::ExactType(RainTypeId::Boolean) => {
                        CheckValue::ExactType(RainTypeId::Boolean)
                    }
                    CheckValue::ExactValue(Value::Boolean(b)) => {
                        CheckValue::ExactValue(Value::Boolean(!b))
                    }
                    // Something has gone in the child, let's not make any assumption
                    _ => CheckValue::Unknown,
                }
            }
            Node::FormatStringLiteral(literal) => {
                for &nid in &literal.nodes {
                    self.check_node(nid, CheckValue::Unknown);
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
                kind: SimpleLiteralKind::Import | SimpleLiteralKind::Stdlib,
                ..
            }) => CheckValue::ExactType(RainTypeId::Closure),
            Node::SimpleLiteral(SimpleLiteral {
                kind: SimpleLiteralKind::ThisFile,
                ..
            }) => self
                .module
                .file
                .as_ref()
                .map(|f| CheckValue::ExactValue(f.clone().to_value()))
                .unwrap_or_default(),
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
            Node::StringLiteral(lit) => {
                let contents = lit.contents.contents(&self.module.src);
                CheckValue::ExactValue(Value::String(Arc::new(super::EscapeReplacer::replace_all(
                    contents,
                ))))
            }
            Node::RawStringLiteral(lit) => {
                let contents = lit.contents.contents(&self.module.src);
                CheckValue::ExactValue(Value::String(Arc::new(contents.to_string())))
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
        if let Some(locals) = &self.locals {
            for (&name, v) in locals {
                captures.insert(name, v.clone());
            }
        }
        CheckCx {
            module: self.module,
            previous: None,
            locals: Some(HashMap::new()),
            captures,
            args: HashMap::new(),
            errors: self.errors,
            node_types: self.node_types,
            declarations: self.declarations,
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
