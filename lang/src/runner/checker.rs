use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use alias::Alias as _;

use crate::{
    ast::{
        AlternateCondition, BinaryOp, BinaryOperatorKind, DeclareName, Namespace, Node, NodeId,
        SimpleLiteral, SimpleLiteralKind,
    },
    ir::{IrModule, LocalDeclarationId},
    local_span::ErrorLocalSpan,
    runner::{
        internal::InternalFunction,
        value::{Closure, ClosureCaptures, RainTypeId, Value},
    },
};

pub struct CheckModuleResult {
    pub errors: Vec<ErrorLocalSpan<CheckError>>,
    pub declarations: HashMap<LocalDeclarationId, Declaration>,
    pub node_types: HashMap<NodeId, CheckValue>,
}

impl CheckModuleResult {
    pub fn check_module(module: &Arc<IrModule>, check_unused: bool) -> Self {
        let mut declaration_names = HashSet::<&str>::new();
        let mut errors = Vec::new();
        for (_, assignment) in module.declaration_assignments() {
            for name_span in assignment.name_spans() {
                let name = name_span.contents(&module.src);
                if !declaration_names.insert(name) {
                    errors.push(name_span.with_error(CheckError::ConflictingDeclaration));
                }
            }
        }
        let mut node_types = HashMap::new();
        let mut declarations = HashMap::new();
        let mut check_cx = CheckCx {
            module: &module.alias(),
            previous: None,
            locals: HashMap::new(),
            captures: HashMap::new(),
            args: HashMap::new(),
            errors: &mut errors,
            node_types: &mut node_types,
            declarations: &mut declarations,
        };
        for id in module.declaration_ids() {
            check_cx.check_declaration(id);
        }
        debug_assert!(
            check_cx.locals.is_empty(),
            "locals cannot be set in declaration scope"
        );
        debug_assert!(
            check_cx.captures.is_empty(),
            "captures cannot be set in declaration scope"
        );
        debug_assert!(
            check_cx.args.is_empty(),
            "args cannot be set in declaration scope"
        );
        debug_assert!(
            check_cx.previous.is_none(),
            "previous cannot be set in declaration scope"
        );
        if check_unused {
            check_cx.check_unused_declarations();
        }
        Self {
            errors,
            declarations,
            node_types,
        }
    }

    pub fn check_node_type(&self, node: NodeId) -> CheckValue {
        self.node_types.get(&node).cloned().unwrap_or_default()
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub enum CheckValue {
    /// No clue, could be anything
    #[default]
    Unknown,
    /// The value be of this concrete type but it is not known what value it is within the type
    ExactType(RainTypeId),
    /// The value should be a type constraint
    UnknownTypeConstraint,
    /// The value should satisfy this type constraint, the closure should not throw
    SatisfiesTypeConstraint(Closure),
    /// The value should be this value
    ExactValue(Value),
    /// The value should be callable (closure or similar) with a certain number of arguments
    Callable {
        arg_types: Vec<Self>,
        return_type: Box<Self>,
    },
    Throwing,
}

impl CheckValue {
    fn exact_type(&self) -> Option<RainTypeId> {
        match self {
            Self::ExactType(rain_type_id) => Some(*rain_type_id),
            Self::ExactValue(value) => Some(value.rain_type_id()),
            _ => None,
        }
    }

    fn type_constraint(&self) -> Option<RainTypeId> {
        match self {
            Self::ExactValue(Value::Type(rain_type_id)) => Some(*rain_type_id),
            _ => None,
        }
    }

    fn into_satisfies_type_constraint(self) -> Self {
        match self {
            Self::ExactValue(Value::Closure(closure)) => Self::SatisfiesTypeConstraint(closure),
            Self::Callable {
                arg_types,
                return_type: _,
            } if arg_types.len() == 1 => Self::Unknown,
            Self::Unknown
            | Self::ExactType(RainTypeId::Closure)
            | Self::UnknownTypeConstraint
            | Self::SatisfiesTypeConstraint(_) => Self::Unknown,
            Self::ExactValue { .. } | Self::Callable { .. } | Self::ExactType { .. } => {
                Self::Unknown
            }
            Self::Throwing => Self::Throwing,
        }
    }
}

struct CheckCx<'a, 'b> {
    module: &'a Arc<IrModule>,
    previous: Option<CheckValue>,
    locals: HashMap<&'a str, CheckValue>,
    captures: HashMap<&'a str, CheckValue>,
    args: HashMap<&'a str, CheckValue>,
    errors: &'b mut Vec<ErrorLocalSpan<CheckError>>,
    node_types: &'b mut HashMap<NodeId, CheckValue>,
    declarations: &'b mut HashMap<LocalDeclarationId, Declaration>,
}

#[derive(Debug, Default)]
pub struct Declaration {
    pub value: CheckValue,
    pub usage: usize,
}

impl CheckCx<'_, '_> {
    fn type_check(&mut self, expected: &CheckValue, actual: &CheckValue) -> Option<CheckError> {
        match (expected, actual) {
            (CheckValue::Unknown | CheckValue::Throwing, _)
            | (_, CheckValue::Unknown | CheckValue::Throwing) => None,
            (CheckValue::ExactType(expected), CheckValue::ExactType(actual)) => {
                if *expected == *actual {
                    None
                } else {
                    Some(CheckError::TypeError(*expected, *actual))
                }
            }
            (CheckValue::ExactType(expected), CheckValue::ExactValue(value)) => {
                if *expected == value.rain_type_id() {
                    None
                } else {
                    Some(CheckError::TypeError(*expected, value.rain_type_id()))
                }
            }
            (CheckValue::UnknownTypeConstraint, CheckValue::ExactType(RainTypeId::Closure)) => {
                // Closures are the only valid type constraint
                None
            }
            (CheckValue::UnknownTypeConstraint, CheckValue::ExactType(type_id)) => {
                Some(CheckError::InvalidTypeConstraint(*type_id))
            }
            (
                CheckValue::UnknownTypeConstraint,
                CheckValue::Callable {
                    arg_types,
                    return_type: _,
                },
            ) => {
                if arg_types.len() == 1 {
                    None
                } else {
                    Some(CheckError::InvalidTypeConstraintWrongArgCount(
                        arg_types.len(),
                    ))
                }
            }
            (
                CheckValue::Callable {
                    arg_types: expected_arg_types,
                    return_type: _,
                },
                CheckValue::Callable {
                    arg_types: actual_arg_types,
                    return_type: _,
                },
            ) => {
                if expected_arg_types.len() == actual_arg_types.len() {
                    None
                } else {
                    Some(CheckError::WrongArgCount(
                        expected_arg_types.len(),
                        actual_arg_types.len(),
                    ))
                }
            }
            (CheckValue::Callable { .. }, _) => todo!(),
            (CheckValue::SatisfiesTypeConstraint(closure), v) => {
                let mut callee_cx = self.callee();
                let m = callee_cx.module;
                let Node::Closure(closure_declare) = m.get(closure.node) else {
                    unreachable!()
                };
                if closure_declare.args.len() != 1 {
                    return Some(CheckError::InvalidTypeConstraintWrongArgCount(
                        closure_declare.args.len(),
                    ));
                }
                callee_cx
                    .args
                    .insert(closure_declare.args[0].name.contents(&m.src), v.clone());
                match callee_cx.check_node(closure_declare.block, CheckValue::Unknown) {
                    CheckValue::Throwing => Some(CheckError::FailedTypeCheck),
                    _ => None,
                }
            }
            (CheckValue::UnknownTypeConstraint, CheckValue::ExactValue(Value::Closure(..))) => {
                // Closure is allowed
                None
            }
            (CheckValue::ExactValue { .. }, _) => todo!(),
            (CheckValue::ExactType { .. }, _) => {
                todo!()
            }
            (CheckValue::UnknownTypeConstraint, CheckValue::ExactValue(value)) => {
                Some(CheckError::InvalidTypeConstraint(value.rain_type_id()))
            }
            (CheckValue::UnknownTypeConstraint, _) => todo!(),
        }
    }

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

    fn check_declaration<'b, 'c>(&'c mut self, id: LocalDeclarationId)
    where
        'c: 'b,
    {
        if self.declarations.get(&id).is_some() {
            return;
        }
        self.declarations.entry(id).or_default();
        let assignment = self.module.get_declaration_assignment(id);
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
                let Some(id) = self.module.find_declaration_by_name(name) else {
                    unreachable!();
                };
                let declaration = self.declarations.entry(id).or_default();
                declaration.value = rhs;
            }
            DeclareName::NamedDestructure(declare) => {
                self.check_node(assignment.expr, CheckValue::Unknown);
                for e in &declare.elements {
                    let name = e.name.contents(&self.module.src);
                    if let Some(type_spec) = &e.type_spec {
                        self.check_node(type_spec.type_expr, CheckValue::Unknown);
                    }
                    let Some(id) = self.module.find_declaration_by_name(name) else {
                        unreachable!();
                    };
                    let declaration = self.declarations.entry(id).or_default();
                    declaration.value = CheckValue::Unknown;
                }
            }
            DeclareName::SequenceDestructure(declare) => {
                self.check_node(assignment.expr, CheckValue::ExactType(RainTypeId::List));
                for e in &declare.elements {
                    let name = e.name.contents(&self.module.src);
                    if let Some(type_spec) = &e.type_spec {
                        self.check_node(type_spec.type_expr, CheckValue::Unknown);
                    }
                    let Some(id) = self.module.find_declaration_by_name(name) else {
                        unreachable!();
                    };
                    let declaration = self.declarations.entry(id).or_default();
                    declaration.value = CheckValue::Unknown;
                }
            }
        }
    }

    fn check_node<'b, 'c>(&'c mut self, nid: NodeId, expected: CheckValue) -> CheckValue
    where
        'c: 'b,
    {
        let mut actual = self.check_node_inner(nid, expected.clone());
        // Check the type
        if let Some(err) = self.type_check(&expected, &actual) {
            self.errors.push(self.module.span(nid).with_error(err));
        }
        if matches!(actual, CheckValue::Unknown) {
            // The expected type knows more about this than the implementation so lets use that
            actual = expected;
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
                if let Some(v) = self.locals.get(ident) {
                    return v.clone();
                }
                if let Some(v) = self.args.get(ident) {
                    return v.clone();
                }
                if let Some(v) = self.captures.get(ident) {
                    return v.clone();
                }
                if let Some(id) = self.module.find_declaration_by_name(ident) {
                    self.check_declaration(id);
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
                    let checked = self.check_node(nid, CheckValue::Unknown);
                    if checked == CheckValue::Throwing {
                        return CheckValue::Throwing;
                    }
                    self.previous = Some(checked);
                }
                self.check_node(*last, expected)
            }
            Node::Closure(closure) => {
                let mut callee_cx = self.callee();
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
                    callee_cx
                        .args
                        .insert(a.name.contents(&callee_cx.module.src), expected);
                }
                let expected_return_type = if let Some(return_type) = &closure.return_type {
                    callee_cx
                        .check_node(return_type.type_expr, CheckValue::UnknownTypeConstraint)
                        .into_satisfies_type_constraint()
                } else {
                    CheckValue::Unknown
                };
                callee_cx.check_node(closure.block, expected_return_type);
                CheckValue::ExactValue(Value::Closure(Closure {
                    captures: ClosureCaptures(Arc::new(
                        self.captures
                            .iter()
                            .map(|(k, v)| (k.to_string(), v.clone()))
                            .collect(),
                    )),
                    module: self.module.id,
                    node: nid,
                }))
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
                    CheckValue::ExactValue(Value::InternalFunction(InternalFunction::Throw)) => {
                        return CheckValue::Throwing;
                    }
                    CheckValue::Callable {
                        arg_types,
                        return_type,
                    } => {
                        if fn_call.args.len() != arg_types.len() {
                            self.errors.push(fn_call.rparen_token.with_error(
                                CheckError::WrongArgCount(arg_types.len(), fn_call.args.len()),
                            ));
                        }
                        for (expected, arg) in arg_types.into_iter().zip(&fn_call.args) {
                            self.check_node(*arg, expected);
                        }
                        return *return_type;
                    }
                    _ => {
                        for &a in &fn_call.args {
                            self.check_node(a, CheckValue::Unknown);
                        }
                    }
                }
                CheckValue::Unknown
            }
            Node::IfCondition(condition) => {
                match self.check_node(
                    condition.condition,
                    CheckValue::ExactType(RainTypeId::Boolean),
                ) {
                    CheckValue::ExactValue(Value::Boolean(true)) => {
                        return self.check_node(condition.then_block, expected);
                    }
                    CheckValue::ExactValue(Value::Boolean(false)) => {
                        return match &condition.alternate {
                            Some(
                                AlternateCondition::IfElseCondition {
                                    else_token: _,
                                    if_condition: alternate,
                                }
                                | AlternateCondition::ElseBlock {
                                    else_token: _,
                                    else_block: alternate,
                                },
                            ) => self.check_node(*alternate, expected),
                            None => CheckValue::ExactValue(Value::Unit),
                        };
                    }
                    _ => {}
                }
                self.check_node(condition.then_block, expected.clone());
                match &condition.alternate {
                    Some(
                        AlternateCondition::IfElseCondition {
                            else_token: _,
                            if_condition: alternate,
                        }
                        | AlternateCondition::ElseBlock {
                            else_token: _,
                            else_block: alternate,
                        },
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
                        self.locals.insert(name, rhs);
                    }
                    DeclareName::NamedDestructure(declare) => {
                        self.check_node(assignment.expr, CheckValue::Unknown);
                        for e in &declare.elements {
                            let name = e.name.contents(&self.module.src);
                            if let Some(type_spec) = &e.type_spec {
                                self.check_node(type_spec.type_expr, CheckValue::Unknown);
                            }
                            self.locals.insert(name, CheckValue::Unknown);
                        }
                    }
                    DeclareName::SequenceDestructure(declare) => {
                        self.check_node(assignment.expr, CheckValue::ExactType(RainTypeId::List));
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
            Node::Namespace(Namespace { left, name, .. }) => {
                if matches!(
                    self.check_node(*left, CheckValue::Unknown).exact_type(),
                    Some(RainTypeId::Internal)
                ) {
                    let name_contents = name.contents(&self.module.src);
                    if let Some(internal_function) =
                        InternalFunction::evaluate_internal_function_name(name_contents)
                    {
                        return CheckValue::ExactValue(Value::InternalFunction(internal_function));
                    }
                    self.errors
                        .push(name.with_error(CheckError::InvalidInternal));
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
                op: BinaryOperatorKind::Equals,
                right,
                ..
            }) => {
                let left = self.check_node(*left, CheckValue::Unknown);
                let right = self.check_node(*right, CheckValue::Unknown);
                match (left, right) {
                    (
                        CheckValue::ExactValue(Value::Type(left)),
                        CheckValue::ExactValue(Value::Type(right)),
                    ) => CheckValue::ExactValue(Value::Boolean(left == right)),
                    (CheckValue::ExactValue(left), CheckValue::ExactValue(right))
                        if left.rain_type_id() != right.rain_type_id() =>
                    {
                        CheckValue::ExactValue(Value::Boolean(false))
                    }
                    _ => CheckValue::ExactType(RainTypeId::Boolean),
                }
            }
            Node::BinaryOp(BinaryOp {
                left,
                op: BinaryOperatorKind::NotEquals,
                right,
                ..
            }) => {
                let left = self.check_node(*left, CheckValue::Unknown);
                let right = self.check_node(*right, CheckValue::Unknown);
                match (left, right) {
                    (
                        CheckValue::ExactValue(Value::Type(left)),
                        CheckValue::ExactValue(Value::Type(right)),
                    ) => CheckValue::ExactValue(Value::Boolean(left != right)),
                    (CheckValue::ExactValue(left), CheckValue::ExactValue(right))
                        if left.rain_type_id() == right.rain_type_id() =>
                    {
                        CheckValue::ExactValue(Value::Boolean(false))
                    }
                    _ => CheckValue::ExactType(RainTypeId::Boolean),
                }
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
            }) => CheckValue::Callable {
                arg_types: vec![CheckValue::ExactType(RainTypeId::String)],
                return_type: Box::new(CheckValue::ExactType(RainTypeId::Module)),
            },
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
        for (&name, v) in &self.locals {
            captures.insert(name, v.clone());
        }
        CheckCx {
            module: self.module,
            previous: None,
            locals: HashMap::new(),
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
    #[error("type error: type constraint must be closure, actual {0}")]
    InvalidTypeConstraint(RainTypeId),
    #[error("type error: type constraint must be closure with 1 argument, actual {0}")]
    InvalidTypeConstraintWrongArgCount(usize),
    #[error("type error: expected {0}, actual {1}")]
    TypeError(RainTypeId, RainTypeId),
    #[error("invalid underscore, no previous statement")]
    InvalidUnderscore,
    #[error("wrong number of arguments: expected {0}, actual {1}")]
    WrongArgCount(usize, usize),
    #[error("failed type check")]
    FailedTypeCheck,
}
