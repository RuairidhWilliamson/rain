pub mod cache;
pub mod checker;
pub mod cx;
pub mod dep;
pub mod dep_list;
pub mod error;
pub mod internal;
pub mod value;

use std::{
    borrow::Cow,
    collections::{HashMap, hash_map::Entry},
    sync::{Arc, LazyLock},
};

use alias::Alias as _;
use error::{ErrorTrace, RunnerError, Throwing};
use indexmap::IndexMap;
use internal::InternalFunction;
use regex::Regex;

use crate::{
    afs::{
        Dir,
        error::PathError,
        local::{entry::LocalFSEntry, file::LocalFile},
    },
    ast::{
        AlternateCondition, Assignment, BinaryOp, BinaryOperatorKind, DeclareName, FnCall,
        FormatStringLiteral, IfCondition, Namespace, Node, NodeId, Not, SimpleLiteral,
        SimpleLiteralKind,
    },
    driver::{DriverTrait, FSTrait, monitoring::Call},
    hash::FileHash,
    ir::{DeclarationId, Rir},
    local_span::LocalSpan,
    runner::{
        cache::{CacheGuardTrait as _, CacheKey, CacheTrait},
        cx::{Cx, StacktraceEntry},
        dep_list::DepList,
        value::{Closure, ClosureCaptures, RainInteger, RainList, RainRecord, RainTypeId, Value},
    },
};

type ResultValue = Result<Value>;
type Result<T, E = ErrorTrace<Throwing>> = core::result::Result<T, E>;

/// Runner represents a lifetime of a single run and makes a lot of assumptions around this
///
/// The main assumptions it makes is:
/// - Local files do not change during its lifetime
pub struct Runner<'a, Driver, Cache> {
    pub ir: &'a mut Rir,
    pub cache: &'a Cache,
    pub driver: &'a Driver,
    pub offline: bool,
    pub seal: bool,
    pub check_unused: bool,
    pub no_exec: bool,
    pub max_call_depth: usize,
    pub local_file_hash_cache: LocalFileHashCache,
}

#[derive(Default)]
pub struct LocalFileHashCache {
    hashes: HashMap<LocalFSEntry, FileHash>,
}

impl LocalFileHashCache {
    pub fn hash<'a>(
        &'a mut self,
        fsentry: LocalFSEntry,
        fs: &impl FSTrait,
    ) -> Result<&'a FileHash, PathError> {
        match self.hashes.entry(fsentry.clone()) {
            Entry::Occupied(entry) => Ok(entry.into_mut()),
            Entry::Vacant(entry) => {
                let file = LocalFile::new_checked(fs, fsentry)?;
                let hash = file.file_hash();
                Ok(entry.insert(hash.clone()))
            }
        }
    }
}

impl<'a, Driver: DriverTrait, Cache: CacheTrait> Runner<'a, Driver, Cache> {
    pub fn new(ir: &'a mut Rir, cache: &'a Cache, driver: &'a Driver) -> Self {
        Self {
            ir,
            cache,
            driver,
            offline: false,
            seal: false,
            check_unused: false,
            no_exec: false,
            max_call_depth: 250,
            local_file_hash_cache: LocalFileHashCache::default(),
        }
    }

    pub fn call_closure(
        &mut self,
        cx: &mut Cx,
        nid: NodeId,
        call_span: LocalSpan,
        closure: &Closure,
        arg_values: Vec<Value>,
    ) -> ResultValue {
        if self.no_exec {
            return Err(cx.err(call_span, RunnerError::NoExec));
        }
        let m = self.ir.get_module(closure.module).alias();
        let Node::Closure(closure_declare) = m.get(closure.node) else {
            unreachable!()
        };
        if closure_declare.args.len() != arg_values.len() {
            return Err(cx.err(
                call_span,
                RunnerError::IncorrectArgs {
                    required: closure_declare.args.len()..=closure_declare.args.len(),
                    actual: arg_values.len(),
                },
            ));
        }
        let cache_key = m.file.as_ref().map(|module| CacheKey::CallClosure {
            captures: closure.captures.clone(),
            module: module.clone(),
            node: closure.node,
            args: arg_values.clone(),
        });
        let mut guard = self
            .cache
            .guard(cache_key, self.driver, &mut self.local_file_hash_cache);
        if let Some((v, deps)) = guard.check() {
            cx.propagate_deps(deps);
            return Ok(v);
        }
        let names = m.find_containing_declaration_name(closure.node);
        let _call = self.driver.call_guard(Call::Closure(names));
        let args = closure_declare
            .args
            .iter()
            .zip(arg_values)
            .map(|(a, v)| (a.name.contents(&m.src), v))
            .collect();
        let mut callee_cx = cx.callee(
            &m,
            args,
            &closure.captures,
            StacktraceEntry {
                m: cx.module.id,
                n: nid,
            },
        );
        for a in &closure_declare.args {
            if let Some(type_spec) = &a.type_spec {
                let v = callee_cx
                    .args
                    .get(a.name.contents(&m.src))
                    .expect("we just put this here")
                    .clone();
                self.evaluate_type_check(&mut callee_cx, &v, type_spec.type_expr)?;
            }
        }
        let value = self.evaluate_node(&mut callee_cx, closure_declare.block)?;
        if let Some(type_spec) = &closure_declare.return_type {
            self.evaluate_type_check(&mut callee_cx, &value, type_spec.type_expr)?;
        }
        cx.propagate_deps(callee_cx.deps.clone());
        guard.put_if_slow(callee_cx.deps, value.clone());

        Ok(value)
    }

    pub fn evaluate_declaration(&mut self, cx: &mut Cx, id: DeclarationId) -> ResultValue {
        let m = self.ir.get_module(id.module_id()).alias();
        let span = m.get_declaration_name_span(id.local_id());
        if self.no_exec {
            return Err(cx.err(span, RunnerError::NoExec));
        }
        let declaration_name = span.contents(&m.src);
        let declaration = m.get_declaration(id.local_id());
        let assignment = m.get_declaration_assignment(id.local_id());
        // If calling into another module check the privacy
        if id.module_id() != cx.module.id && declaration.pub_token.is_none() {
            let span = m.get_declaration_name_span(id.local_id());
            return Err(span
                .with_module(id.module_id())
                .with_error(RunnerError::PrivateDeclaration.into())
                .with_trace(cx.stacktrace.clone()));
        }
        let stacktrace = cx.stacktrace.clone();
        let mut callee_cx = Cx::new(&m, cx.call_depth + 1, HashMap::new(), stacktrace);
        let key = m.file.as_ref().map(|file| cache::CacheKey::Declaration {
            module: file.clone(),
            name: declaration_name.to_owned(),
        });
        let mut guard = self
            .cache
            .guard(key, self.driver, &mut self.local_file_hash_cache);
        if let Some((v, deps)) = guard.check() {
            cx.propagate_deps(deps);
            return Ok(v);
        }
        let _call = self
            .driver
            .call_guard(Call::Declaration(declaration_name.to_string()));
        let result = self.evaluate_node(&mut callee_cx, assignment.expr)?;
        let value = match &assignment.name {
            DeclareName::Single(single) => {
                if let Some(type_spec) = &single.type_spec {
                    self.evaluate_type_check(&mut callee_cx, &result, type_spec.type_expr)?;
                }
                result
            }
            DeclareName::NamedDestructure(_) | DeclareName::SequenceDestructure(_) => {
                let span = m.get_declaration_name_span(id.local_id());
                let name = span.contents(&m.src);
                let Some(value) = self.evaluate_named_index(cx, &result, span, name)? else {
                    return Err(cx.nid_err(
                        assignment.expr,
                        RunnerError::IndexKeyNotFound(name.to_owned()),
                    ));
                };
                if let Some(type_spec) = m.get_declaration_type_spec(id.local_id()).as_ref() {
                    self.evaluate_type_check(&mut callee_cx, &value, type_spec.type_expr)?;
                }
                value
            }
        };
        cx.propagate_deps(callee_cx.deps.clone());
        guard.put_if_slow(callee_cx.deps, value.clone());
        Ok(value)
    }

    fn evaluate_node(&mut self, cx: &mut Cx, nid: NodeId) -> ResultValue {
        match cx.module.get(nid) {
            Node::Closure(_) => Ok(evaluate_closure_definition(cx, nid)),
            Node::Block(block) => {
                for nid in &block.statements {
                    let v = self.evaluate_node(cx, *nid)?;
                    cx.previous_line = Some(v);
                }
                Ok(cx.previous_line.clone().unwrap_or(Value::Unit))
            }
            Node::IfCondition(if_condition) => self.evaluate_if_condition(cx, if_condition),
            Node::FnCall(fn_call) => self.evaluate_fn_call(cx, nid, fn_call),
            Node::Assignment(assignment) => self.evaluate_assignment(cx, assignment),
            Node::BinaryOp(binary_op) => self.evaluate_binary_op(cx, binary_op),
            Node::Ident(tls) => self
                .resolve_ident(cx, tls.0.contents(&cx.module.src))?
                .ok_or_else(|| cx.err(tls.0, RunnerError::UnknownIdent)),
            Node::SimpleLiteral(SimpleLiteral {
                kind: SimpleLiteralKind::True,
                ..
            }) => Ok(Value::Boolean(true)),
            Node::SimpleLiteral(SimpleLiteral {
                kind: SimpleLiteralKind::False,
                ..
            }) => Ok(Value::Boolean(false)),
            Node::SimpleLiteral(SimpleLiteral {
                kind: SimpleLiteralKind::Internal,
                ..
            }) => Ok(Value::Internal),
            Node::SimpleLiteral(SimpleLiteral {
                span,
                kind: SimpleLiteralKind::Import,
            }) => self.import_sugar(cx, nid, *span),
            Node::SimpleLiteral(SimpleLiteral {
                span,
                kind: SimpleLiteralKind::Stdlib,
            }) => self.stdlib_sugar(cx, nid, *span),
            Node::SimpleLiteral(SimpleLiteral {
                span,
                kind: SimpleLiteralKind::ThisFile,
            }) => self.this_file_sugar(cx, nid, *span),
            Node::SimpleLiteral(SimpleLiteral {
                span,
                kind: SimpleLiteralKind::Underscore,
            }) => cx.previous_line.clone().ok_or_else(|| {
                cx.err(span, RunnerError::Makeshift("no previous statement".into()))
            }),
            Node::StringLiteral(lit) => {
                let contents = lit.contents.contents(&cx.module.src);
                Ok(Value::String(Arc::new(EscapeReplacer::replace_all(
                    contents,
                ))))
            }
            Node::RawStringLiteral(lit) => {
                let contents = lit.contents.contents(&cx.module.src);
                Ok(Value::String(Arc::new(contents.to_string())))
            }
            Node::FormatStringLiteral(lit) => self.evaluate_format_string(cx, lit),
            Node::IntegerLiteral(tls) => Ok(Value::Integer(Arc::new(RainInteger(
                tls.0
                    .contents(&cx.module.src)
                    .parse::<num_bigint::BigInt>()
                    .map_err(|_| cx.err(tls.0, RunnerError::InvalidIntegerLiteral))?,
            )))),
            Node::Record(record) => {
                let mut builder = IndexMap::new();
                for e in &record.fields {
                    builder.insert(
                        e.key.contents(&cx.module.src).to_owned(),
                        self.evaluate_node(cx, e.value)?,
                    );
                }
                Ok(Value::Record(Arc::new(RainRecord(builder))))
            }
            Node::List(list) => {
                let mut builder = Vec::new();
                for e in &list.elements {
                    builder.push(self.evaluate_node(cx, e.value)?);
                }
                Ok(Value::List(Arc::new(RainList(builder))))
            }
            Node::Not(Not { exclamation, inner }) => {
                let inner_value = self.evaluate_node(cx, *inner)?;
                match inner_value {
                    Value::Boolean(b) => Ok(Value::Boolean(!b)),
                    _ => Err(cx.err(
                        exclamation,
                        RunnerError::ExpectedType {
                            actual: inner_value.rain_type_id(),
                            expected: Cow::Borrowed(&[RainTypeId::Boolean]),
                        },
                    )),
                }
            }
            Node::Namespace(namespace) => self.evaluate_dot_operator(cx, namespace),
        }
    }

    fn evaluate_format_string(
        &mut self,
        cx: &mut Cx<'_>,
        lit: &FormatStringLiteral,
    ) -> ResultValue {
        let mut out = String::new();
        let mut start = lit.contents.start;
        for nid in &lit.nodes {
            let span = cx.module.span(*nid);
            out.push_str(&EscapeReplacer::replace_all(
                LocalSpan {
                    start,
                    end: span.start - 2,
                }
                .contents(&cx.module.src),
            ));
            start = span.end + 1;
            let v = self.evaluate_node(cx, *nid)?;
            out.push_str(&self.stringify_value(cx, *nid, &v)?);
        }
        out.push_str(&EscapeReplacer::replace_all(
            LocalSpan {
                start,
                end: lit.contents.end,
            }
            .contents(&cx.module.src),
        ));
        Ok(Value::String(Arc::new(out)))
    }

    fn evaluate_assignment(&mut self, cx: &mut Cx, assignment: &Assignment) -> ResultValue {
        let v = self.evaluate_node(cx, assignment.expr)?;
        match &assignment.name {
            DeclareName::Single(declare_name_single) => {
                let name = declare_name_single.name.contents(&cx.module.src);
                if let Some(type_spec) = &declare_name_single.type_spec {
                    self.evaluate_type_check(cx, &v, type_spec.type_expr)?;
                }
                cx.locals.insert(name, v);
                Ok(Value::Unit)
            }
            DeclareName::NamedDestructure(declare_name_destructure) => {
                for name_element in &declare_name_destructure.elements {
                    let name = name_element.name.contents(&cx.module.src);
                    let Some(value) = self.evaluate_named_index(cx, &v, name_element.name, name)?
                    else {
                        return Err(cx.nid_err(
                            assignment.expr,
                            RunnerError::IndexKeyNotFound(name.to_owned()),
                        ));
                    };

                    if let Some(type_spec) = &name_element.type_spec {
                        self.evaluate_type_check(cx, &value, type_spec.type_expr)?;
                    }
                    cx.locals.insert(name, value);
                }
                Ok(Value::Unit)
            }
            DeclareName::SequenceDestructure(declare_name_destructure) => {
                for (index, name_element) in declare_name_destructure.elements.iter().enumerate() {
                    let name = name_element.name.contents(&cx.module.src);
                    let Some(value) = Self::evaluate_index(cx, &v, name_element.name, index)?
                    else {
                        return Err(cx.nid_err(
                            assignment.expr,
                            RunnerError::IndexKeyNotFound(name.to_owned()),
                        ));
                    };

                    if let Some(type_spec) = &name_element.type_spec {
                        self.evaluate_type_check(cx, &value, type_spec.type_expr)?;
                    }
                    cx.locals.insert(name, value);
                }
                Ok(Value::Unit)
            }
        }
    }

    fn resolve_ident(&mut self, cx: &mut Cx, ident: &str) -> Result<Option<Value>> {
        if let Some(v) = cx.locals.get(ident) {
            return Ok(Some(v.clone()));
        }
        if let Some(v) = cx.args.get(ident) {
            return Ok(Some(v.clone()));
        }
        if let Some(v) = cx.captures.0.get(ident) {
            return Ok(Some(v.clone()));
        }
        if let Some(declaration_id) = self.ir.resolve_global_declaration(cx.module.id, ident) {
            return Ok(Some(self.evaluate_declaration(cx, declaration_id)?));
        }
        Ok(None)
    }

    fn evaluate_fn_call(&mut self, cx: &mut Cx, nid: NodeId, fn_call: &FnCall) -> ResultValue {
        let v = self.evaluate_node(cx, fn_call.callee)?;
        let arg_values: Vec<(NodeId, Value)> = fn_call
            .args
            .iter()
            .map(|a| Ok((*a, self.evaluate_node(cx, *a)?)))
            .collect::<Result<_, _>>()?;
        let call_span = fn_call.lparen_token + fn_call.rparen_token;
        self.call_function_like(cx, nid, &v, call_span, arg_values)
    }

    fn call_function_like(
        &mut self,
        cx: &mut Cx,
        nid: NodeId,
        function_value: &Value,
        call_span: LocalSpan,
        arg_values: Vec<(NodeId, Value)>,
    ) -> ResultValue {
        if cx.call_depth >= self.max_call_depth {
            return Err(cx.err(call_span, RunnerError::MaxCallDepth));
        }
        match &function_value {
            Value::Closure(closure) => {
                let arg_values: Vec<_> = arg_values.into_iter().map(|(_, v)| v).collect();
                self.call_closure(cx, nid, call_span, closure, arg_values)
            }
            Value::InternalFunction(f) => {
                let cache_key = CacheKey::InternalFunction {
                    func: *f,
                    args: arg_values.iter().map(|(_, v)| v.clone()).collect(),
                };
                let mut guard = self.cache.guard(
                    Some(cache_key),
                    self.driver,
                    &mut self.local_file_hash_cache,
                );
                if let Some((v, deps)) = guard.check() {
                    cx.propagate_deps(deps);
                    return Ok(v);
                }
                let _call = self.driver.call_guard(Call::Internal(*f));
                log::trace!("internal function call {f:?} {arg_values:?}");
                let mut deps = DepList::new();
                let mut cache_hint = true;
                let internal_cx = internal::InternalCx {
                    func: *f,
                    runner: self,
                    caller_cx: cx,
                    nid,
                    arg_values,
                    call_span,
                    deps: &mut deps,
                    cache_hint: &mut cache_hint,
                };
                let result = internal_cx.call_internal_function()?;
                cx.propagate_deps(deps.clone());
                if cache_hint {
                    guard.put(deps, result.clone());
                } else {
                    guard.put_if_slow(deps, result.clone());
                }
                Ok(result)
            }
            _ => Err(cx.err(
                call_span,
                RunnerError::ExpectedType {
                    actual: function_value.rain_type_id(),
                    expected: Cow::Borrowed(&[RainTypeId::InternalFunction, RainTypeId::Closure]),
                },
            )),
        }
    }

    fn evaluate_type_check(
        &mut self,
        cx: &mut Cx<'_>,
        v: &Value,
        type_spec_nid: NodeId,
    ) -> Result<(), ErrorTrace<Throwing>> {
        let type_spec_value = self.evaluate_node(cx, type_spec_nid)?;
        match type_spec_value {
            Value::Type(expected_type) => {
                if v.rain_type_id() != expected_type {
                    return Err(cx.nid_err(
                        type_spec_nid,
                        RunnerError::ExpectedType {
                            actual: v.rain_type_id(),
                            expected: Cow::Owned(vec![expected_type]),
                        },
                    ));
                }
                Ok(())
            }
            Value::Closure(_) => {
                let result = self.call_function_like(
                    cx,
                    type_spec_nid,
                    &type_spec_value,
                    cx.module.span(type_spec_nid),
                    vec![(type_spec_nid, v.clone())],
                )?;
                match result {
                    Value::Boolean(ok) => {
                        if !ok {
                            return Err(cx.nid_err(
                                type_spec_nid,
                                RunnerError::ExpectedType {
                                    actual: v.rain_type_id(),
                                    // FIXME: We have no way to know what types would work here :(
                                    expected: Cow::Owned(vec![]),
                                },
                            ));
                        }
                        Ok(())
                    }
                    _ => Err(cx.nid_err(
                        type_spec_nid,
                        RunnerError::ExpectedType {
                            actual: type_spec_value.rain_type_id(),
                            expected: Cow::Borrowed(&[RainTypeId::Boolean]),
                        },
                    )),
                }
            }
            _ => Err(cx.nid_err(
                type_spec_nid,
                RunnerError::ExpectedType {
                    actual: type_spec_value.rain_type_id(),
                    expected: Cow::Borrowed(&[RainTypeId::Type, RainTypeId::Closure]),
                },
            )),
        }
    }

    fn evaluate_binary_op(&mut self, cx: &mut Cx, op: &BinaryOp) -> ResultValue {
        let left = self.evaluate_node(cx, op.left)?;
        let right = self.evaluate_node(cx, op.right)?;

        match (left, op.op, right) {
            (Value::String(left), BinaryOperatorKind::Addition, Value::String(right)) => {
                Ok(Value::String(Arc::new(left.to_string() + &**right)))
            }
            (Value::List(left), BinaryOperatorKind::Addition, Value::List(right)) => {
                let mut v = left.0.clone();
                v.append(&mut right.0.clone());
                Ok(Value::List(Arc::new(RainList(v))))
            }
            (Value::Integer(left), BinaryOperatorKind::Addition, Value::Integer(right)) => {
                Ok(Value::Integer(Arc::new(RainInteger(&left.0 + &right.0))))
            }
            (Value::Integer(left), BinaryOperatorKind::Subtraction, Value::Integer(right)) => {
                Ok(Value::Integer(Arc::new(RainInteger(&left.0 - &right.0))))
            }
            (Value::Integer(left), BinaryOperatorKind::Multiplication, Value::Integer(right)) => {
                Ok(Value::Integer(Arc::new(RainInteger(&left.0 * &right.0))))
            }
            (Value::Integer(left), BinaryOperatorKind::Division, Value::Integer(right)) => {
                Ok(Value::Integer(Arc::new(RainInteger(&left.0 / &right.0))))
            }
            (Value::Boolean(left), BinaryOperatorKind::LogicalAnd, Value::Boolean(right)) => {
                Ok(Value::Boolean(left && right))
            }
            (Value::Boolean(left), BinaryOperatorKind::LogicalOr, Value::Boolean(right)) => {
                Ok(Value::Boolean(left || right))
            }
            (Value::Integer(left), BinaryOperatorKind::Equals, Value::Integer(right)) => {
                Ok(Value::Boolean(left.0 == right.0))
            }
            (Value::Integer(left), BinaryOperatorKind::NotEquals, Value::Integer(right)) => {
                Ok(Value::Boolean(left.0 != right.0))
            }
            (Value::String(left), BinaryOperatorKind::Equals, Value::String(right)) => {
                Ok(Value::Boolean(left == right))
            }
            (Value::String(left), BinaryOperatorKind::NotEquals, Value::String(right)) => {
                Ok(Value::Boolean(left != right))
            }
            (Value::Integer(left), BinaryOperatorKind::LessThan, Value::Integer(right)) => {
                Ok(Value::Boolean(left.0 < right.0))
            }
            (Value::Integer(left), BinaryOperatorKind::GreaterThan, Value::Integer(right)) => {
                Ok(Value::Boolean(left.0 > right.0))
            }
            (Value::Integer(left), BinaryOperatorKind::LessThanEquals, Value::Integer(right)) => {
                Ok(Value::Boolean(left.0 <= right.0))
            }
            (
                Value::Integer(left),
                BinaryOperatorKind::GreaterThanEquals,
                Value::Integer(right),
            ) => Ok(Value::Boolean(left.0 >= right.0)),
            (Value::Unit, BinaryOperatorKind::Equals, Value::Unit) => Ok(Value::Boolean(true)),
            (Value::Unit, BinaryOperatorKind::NotEquals, Value::Unit) => Ok(Value::Boolean(false)),
            (left, BinaryOperatorKind::Equals, right)
                if left.rain_type_id() != right.rain_type_id() =>
            {
                Ok(Value::Boolean(false))
            }
            (left, BinaryOperatorKind::NotEquals, right)
                if left.rain_type_id() != right.rain_type_id() =>
            {
                Ok(Value::Boolean(true))
            }
            (Value::Type(left), BinaryOperatorKind::Equals, Value::Type(right)) => {
                Ok(Value::Boolean(left == right))
            }
            (Value::Type(left), BinaryOperatorKind::NotEquals, Value::Type(right)) => {
                Ok(Value::Boolean(left != right))
            }
            (left, _, right) => {
                log::error!("invalid binary op usage {left:?} {op:?} {right:?}");
                Err(cx.err(op.op_span, RunnerError::InvalidBinaryOp))
            }
        }
    }

    fn evaluate_named_index(
        &mut self,
        cx: &mut Cx,
        value: &Value,
        span: LocalSpan,
        name: &str,
    ) -> Result<Option<Value>> {
        match value {
            Value::Module(module_value) => {
                let Some(did) = self.ir.resolve_global_declaration(*module_value, name) else {
                    return Ok(None);
                };
                Ok(Some(self.evaluate_declaration(cx, did)?))
            }
            Value::Internal => Ok(InternalFunction::evaluate_internal_function_name(name)
                .map(Value::InternalFunction)),
            Value::Record(record_value) => Ok(record_value.0.get(name).cloned()),
            _ => Err(cx.err(
                span,
                RunnerError::ExpectedType {
                    actual: value.rain_type_id(),
                    expected: Cow::Borrowed(&[
                        RainTypeId::Module,
                        RainTypeId::Internal,
                        RainTypeId::Record,
                    ]),
                },
            )),
        }
    }

    fn evaluate_index(
        cx: &mut Cx,
        value: &Value,
        span: LocalSpan,
        index: usize,
    ) -> Result<Option<Value>> {
        match value {
            Value::List(list) => Ok(list.0.get(index).cloned()),
            _ => Err(cx.err(
                span,
                RunnerError::ExpectedType {
                    actual: value.rain_type_id(),
                    expected: Cow::Borrowed(&[RainTypeId::List]),
                },
            )),
        }
    }

    fn evaluate_dot_operator(&mut self, cx: &mut Cx, namespace: &Namespace) -> ResultValue {
        let left = self.evaluate_node(cx, namespace.left)?;
        let name_contents = namespace.name.contents(&cx.module.src);
        let Some(value) = self.evaluate_named_index(cx, &left, namespace.name, name_contents)?
        else {
            return Err(cx.err(
                namespace.name,
                RunnerError::IndexKeyNotFound(name_contents.to_owned()),
            ));
        };
        Ok(value)
    }

    fn evaluate_if_condition(&mut self, cx: &mut Cx, if_condition: &IfCondition) -> ResultValue {
        let condition_value = self.evaluate_node(cx, if_condition.condition)?;
        let Value::Boolean(condition_bool) = condition_value else {
            return Err(cx.nid_err(
                if_condition.condition,
                RunnerError::ExpectedType {
                    actual: condition_value.rain_type_id(),
                    expected: Cow::Borrowed(&[RainTypeId::Boolean]),
                },
            ));
        };
        if condition_bool {
            self.evaluate_node(cx, if_condition.then_block)
        } else {
            match if_condition.alternate {
                Some(AlternateCondition::IfElseCondition(if_condition)) => {
                    self.evaluate_node(cx, if_condition)
                }
                Some(AlternateCondition::ElseBlock(block)) => self.evaluate_node(cx, block),
                None => Ok(Value::Unit),
            }
        }
    }

    fn import_sugar(&mut self, cx: &mut Cx, nid: NodeId, call_span: LocalSpan) -> ResultValue {
        let embed_value = self.call_function_like(
            cx,
            nid,
            &Value::InternalFunction(InternalFunction::Embed),
            call_span,
            Vec::new(),
        )?;
        let module_file = self.call_function_like(
            cx,
            nid,
            &Value::InternalFunction(InternalFunction::ModuleFile),
            call_span,
            Vec::new(),
        )?;
        let Value::Module(embed_mid) = embed_value else {
            return Err(cx.err(
                call_span,
                RunnerError::ExpectedType {
                    actual: embed_value.rain_type_id(),
                    expected: (&[RainTypeId::Module]).into(),
                },
            ));
        };
        let Some(did) = self
            .ir
            .resolve_global_declaration(embed_mid, "import_sugar_implementation")
        else {
            return Err(cx.err(call_span, RunnerError::UnknownIdent));
        };
        let import_closure_generator = self.evaluate_declaration(cx, did)?;
        self.call_function_like(
            cx,
            nid,
            &import_closure_generator,
            call_span,
            vec![(nid, module_file)],
        )
    }

    fn stdlib_sugar(&mut self, cx: &mut Cx, nid: NodeId, call_span: LocalSpan) -> ResultValue {
        let embed_value = self.call_function_like(
            cx,
            nid,
            &Value::InternalFunction(InternalFunction::Embed),
            call_span,
            Vec::new(),
        )?;
        let Value::Module(embed_mid) = embed_value else {
            return Err(cx.err(
                call_span,
                RunnerError::ExpectedType {
                    actual: embed_value.rain_type_id(),
                    expected: (&[RainTypeId::Module]).into(),
                },
            ));
        };
        let Some(did) = self
            .ir
            .resolve_global_declaration(embed_mid, "stdlib_sugar_implementation")
        else {
            return Err(cx.err(call_span, RunnerError::UnknownIdent));
        };
        self.evaluate_declaration(cx, did)
    }

    fn this_file_sugar(&mut self, cx: &mut Cx, nid: NodeId, call_span: LocalSpan) -> ResultValue {
        self.call_function_like(
            cx,
            nid,
            &Value::InternalFunction(InternalFunction::ModuleFile),
            call_span,
            Vec::new(),
        )
    }

    fn stringify_value(&self, cx: &mut Cx, nid: NodeId, v: &Value) -> Result<String> {
        match v {
            Value::String(s) => Ok(s.as_ref().clone()),
            Value::GeneratedFile(f) => Ok(self
                .driver
                .resolve_fs_entry(f.fsinner().into())
                .display()
                .to_string()),
            Value::LocalFile(f) => Ok(self
                .driver
                .resolve_fs_entry(f.fsinner().into())
                .display()
                .to_string()),
            Value::GeneratedFSArea(area) => Ok(self
                .driver
                .resolve_fs_entry(Dir::root(area.as_ref().into()).fsinner())
                .display()
                .to_string()),
            Value::LocalFSArea(area) => Ok(self
                .driver
                .resolve_fs_entry(Dir::root(area.as_ref().into()).fsinner())
                .display()
                .to_string()),
            Value::GeneratedDir(d) => Ok(self
                .driver
                .resolve_fs_entry(d.fsinner().into())
                .display()
                .to_string()),
            Value::LocalDir(d) => Ok(self
                .driver
                .resolve_fs_entry(d.fsinner().into())
                .display()
                .to_string()),
            Value::EscapeFile(f) => Ok(format!("{}", f.0.display())),
            Value::Integer(i) => Ok(i.to_string()),
            Value::Boolean(b) => Ok(b.to_string()),
            _ => Err(cx.nid_err(
                nid,
                RunnerError::ExpectedType {
                    actual: v.rain_type_id(),
                    expected: Cow::Borrowed(&[
                        RainTypeId::String,
                        RainTypeId::GeneratedFile,
                        RainTypeId::GeneratedDir,
                        RainTypeId::GeneratedFSArea,
                        RainTypeId::LocalFSArea,
                        RainTypeId::LocalFile,
                        RainTypeId::LocalDir,
                        RainTypeId::EscapeFile,
                        RainTypeId::Integer,
                        RainTypeId::Boolean,
                    ]),
                },
            )),
        }
    }
}

fn evaluate_closure_definition(cx: &mut Cx, nid: NodeId) -> Value {
    let mut captures = HashMap::<String, Value>::new();
    for (k, v) in cx.captures.0.iter() {
        captures.insert(k.clone(), v.clone());
    }
    for (k, v) in &cx.args {
        captures.insert(k.to_string(), v.clone());
    }
    for (k, v) in &cx.locals {
        captures.insert(k.to_string(), v.clone());
    }
    Value::Closure(Closure {
        captures: ClosureCaptures(Arc::new(captures)),
        module: cx.module.id,
        node: nid,
    })
}

struct EscapeReplacer;

impl EscapeReplacer {
    fn regex() -> &'static regex::Regex {
        static REGEX: LazyLock<regex::Regex> =
            LazyLock::new(|| Regex::new("\\\\.").expect("compile regex"));
        &REGEX
    }

    fn replace_all(contents: &str) -> String {
        Self::regex().replace_all(contents, Self).into_owned()
    }
}

impl regex::Replacer for EscapeReplacer {
    fn replace_append(&mut self, caps: &regex::Captures<'_>, dst: &mut String) {
        let s = &caps[0];
        let replaced = match s.chars().last().expect("last char") {
            '"' => "\"",
            'n' => "\n",
            't' => "\t",
            'r' => "\r",
            '\\' => "\\",
            '0' => "\0",
            _ => s,
        };
        dst.push_str(replaced);
    }
}
