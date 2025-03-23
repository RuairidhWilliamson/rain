pub mod cache;
pub mod error;
pub mod hash;
pub mod internal;
pub mod value;
pub mod value_impl;

use std::{collections::HashMap, sync::Arc, time::Instant};

use error::{RunnerError, Throwing};
use indexmap::IndexMap;
use internal::InternalFunction;
use value::{RainTypeId, Value, ValueInner};
use value_impl::{Module, RainFunction, RainInteger, RainInternal, RainList, RainRecord};

use crate::{
    ast::{AlternateCondition, BinaryOp, BinaryOperatorKind, FnCall, IfCondition, Node, NodeId},
    driver::DriverTrait,
    ir::{DeclarationId, IrModule, Rir},
    local_span::LocalSpan,
    span::ErrorSpan,
};

const MAX_CALL_DEPTH: usize = 250;

type ResultValue = Result<Value>;
type Result<T, E = ErrorSpan<Throwing>> = core::result::Result<T, E>;

pub struct Cx<'a> {
    module: &'a Arc<IrModule>,
    call_depth: usize,
    locals: HashMap<&'a str, Value>,
    args: HashMap<&'a str, Value>,
}

impl<'a> Cx<'a> {
    fn new(module: &'a Arc<IrModule>) -> Self {
        Self {
            module,
            call_depth: 0,
            args: HashMap::new(),
            locals: HashMap::new(),
        }
    }

    fn err(&self, s: impl Into<LocalSpan>, err: RunnerError) -> ErrorSpan<Throwing> {
        s.into().with_module(self.module.id).with_error(err.into())
    }

    fn nid_err(&self, nid: NodeId, err: RunnerError) -> ErrorSpan<Throwing> {
        self.module
            .span(nid)
            .with_module(self.module.id)
            .with_error(err.into())
    }
}

pub struct Runner<'a, D> {
    pub ir: &'a mut Rir,
    pub cache: &'a mut cache::Cache,
    pub driver: &'a D,
}

impl<'a, D: DriverTrait> Runner<'a, D> {
    pub fn new(rir: &'a mut Rir, cache: &'a mut cache::Cache, driver: &'a D) -> Self {
        Self {
            ir: rir,
            cache,
            driver,
        }
    }

    pub fn evaluate_and_call(&mut self, id: DeclarationId) -> ResultValue {
        let v = self.evaluate_declaration(id)?;
        let Some(f) = v.downcast_ref::<RainFunction>() else {
            return Ok(v);
        };
        let m = &Arc::clone(self.ir.get_module(f.id.module_id()));
        let nid = m.get_declaration(f.id.local_id());
        let node = m.get(nid);
        match node {
            Node::FnDeclare(fn_declare) => {
                if !fn_declare.args.is_empty() {
                    return Err(fn_declare.rparen_token.span.with_module(m.id).with_error(
                        RunnerError::IncorrectArgs {
                            required: fn_declare.args.len()..=fn_declare.args.len(),
                            actual: 0,
                        }
                        .into(),
                    ));
                }
                let mut cx = Cx {
                    module: m,
                    call_depth: 0,
                    args: HashMap::new(),
                    locals: HashMap::new(),
                };
                self.evaluate_node(&mut cx, fn_declare.block)
            }
            _ => unreachable!(),
        }
    }

    pub fn evaluate_declaration(&mut self, id: DeclarationId) -> ResultValue {
        let m = &Arc::clone(self.ir.get_module(id.module_id()));
        let nid = m.get_declaration(id.local_id());
        let node = m.get(nid);
        match node {
            Node::LetDeclare(let_declare) => self.evaluate_node(&mut Cx::new(m), let_declare.expr),
            Node::FnDeclare(_) => Ok(Value::new(RainFunction { id })),
            _ => unreachable!(),
        }
    }

    fn evaluate_node(&mut self, cx: &mut Cx, nid: NodeId) -> ResultValue {
        match cx.module.get(nid) {
            Node::ModuleRoot(_) => {
                panic!("can't evaluate module root")
            }
            Node::LetDeclare(_) => panic!("can't evaluate let declare"),
            Node::FnDeclare(_) => panic!("can't evaluate fn declare"),
            Node::Block(block) => {
                let mut prev = None;
                for nid in &block.statements {
                    let v = self.evaluate_node(cx, *nid)?;
                    prev = Some(v);
                }
                Ok(prev.unwrap_or_else(value_impl::get_unit))
            }
            Node::IfCondition(if_condition) => self.evaluate_if_condition(cx, if_condition),
            Node::FnCall(fn_call) => self.evaluate_fn_call(cx, nid, fn_call),
            Node::Assignment(assignment) => {
                let v = self.evaluate_node(cx, assignment.expr)?;
                let name = assignment.name.span.contents(&cx.module.src);
                cx.locals.insert(name, v);
                Ok(value_impl::get_unit())
            }
            Node::BinaryOp(binary_op) => self.evaluate_binary_op(cx, binary_op),
            Node::Ident(tls) => self
                .resolve_ident(cx, tls.0.span.contents(&cx.module.src))?
                .ok_or_else(|| {
                    tls.0
                        .span
                        .with_module(cx.module.id)
                        .with_error(RunnerError::UnknownIdent.into())
                }),
            Node::InternalLiteral(_) => Ok(Value::new(RainInternal)),
            Node::StringLiteral(lit) => match lit.prefix() {
                Some(crate::tokens::StringLiteralPrefix::Format) => {
                    log::info!("{lit:?}");
                    let contents = lit.content_span().contents(&cx.module.src);
                    todo!("format strings not implemented: {contents}")
                }
                None => {
                    let contents = lit.content_span().contents(&cx.module.src);
                    // TODO: Improve escaping
                    let re = regex::Regex::new("\\\\.").expect("compile regex");
                    let contents = re.replace_all(contents, |c: &regex::Captures<'_>| -> &str {
                        match c[0].chars().last().expect("last char") {
                            '"' => "\"",
                            'n' => "\n",
                            't' => "\t",
                            c => todo!("escaping not implemented for {c}"),
                        }
                    });
                    Ok(Value::new(contents.to_string()))
                }
            },
            Node::IntegerLiteral(tls) => Ok(Value::new(
                tls.0
                    .span
                    .contents(&cx.module.src)
                    .parse::<RainInteger>()
                    .map_err(|_| cx.err(tls.0, RunnerError::InvalidIntegerLiteral))?,
            )),
            Node::TrueLiteral(_) => Ok(Value::new(true)),
            Node::FalseLiteral(_) => Ok(Value::new(false)),
            Node::Record(record) => {
                let mut builder = IndexMap::new();
                for e in &record.fields {
                    builder.insert(
                        e.key.span.contents(&cx.module.src).to_owned(),
                        self.evaluate_node(cx, e.value)?,
                    );
                }
                Ok(Value::new(RainRecord(builder)))
            }
            Node::List(list) => {
                let mut builder = Vec::new();
                for e in &list.elements {
                    builder.push(self.evaluate_node(cx, e.value)?);
                }
                Ok(Value::new(RainList(builder)))
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
        let Some(declaration_id) = self.ir.resolve_global_declaration(cx.module.id, ident) else {
            return Ok(None);
        };
        Ok(Some(self.evaluate_declaration(declaration_id)?))
    }

    fn evaluate_fn_call(&mut self, cx: &mut Cx, nid: NodeId, fn_call: &FnCall) -> ResultValue {
        let v = self.evaluate_node(cx, fn_call.callee)?;
        let v_type = v.rain_type_id();
        match v_type {
            RainTypeId::Function => {
                if cx.call_depth >= MAX_CALL_DEPTH {
                    return Err(cx.err(fn_call.lparen_token, RunnerError::MaxCallDepth));
                }
                let Some(f) = v.downcast_ref::<RainFunction>() else {
                    unreachable!();
                };
                let arg_values: Vec<Value> = fn_call
                    .args
                    .iter()
                    .map(|a| self.evaluate_node(cx, *a))
                    .collect::<Result<_, _>>()?;
                let key = cache::CacheKey::Declaration {
                    declaration: f.id,
                    args: arg_values.clone(),
                };

                if let Some(v) = self.cache.get_value(&key) {
                    return Ok(v);
                }
                let start = Instant::now();
                let m = &Arc::clone(self.ir.get_module(f.id.module_id()));
                let nid = m.get_declaration(f.id.local_id());
                let node = m.get(nid);
                let Node::FnDeclare(fn_declare) = node else {
                    unreachable!();
                };
                let function_name = fn_declare.name.span.contents(&m.src);
                self.driver.enter_call(function_name);
                if fn_declare.args.len() != fn_call.args.len() {
                    return Err(cx.err(
                        fn_call.rparen_token,
                        RunnerError::IncorrectArgs {
                            required: fn_declare.args.len()..=fn_declare.args.len(),
                            actual: fn_call.args.len(),
                        },
                    ));
                }
                let args = fn_declare
                    .args
                    .iter()
                    .zip(arg_values)
                    .map(|(a, v)| (a.name.span.contents(&m.src), v))
                    .collect();
                let mut cx = Cx {
                    module: m,
                    call_depth: cx.call_depth + 1,
                    args,
                    locals: HashMap::new(),
                };
                let result = self.evaluate_node(&mut cx, fn_declare.block)?;
                self.driver.exit_call(function_name);
                self.cache.put(key, start.elapsed(), None, result.clone());
                Ok(result)
            }
            RainTypeId::InternalFunction => {
                let Some(f) = v.downcast_ref::<InternalFunction>() else {
                    unreachable!()
                };
                let arg_values: Vec<(NodeId, Value)> = fn_call
                    .args
                    .iter()
                    .map(|&a| Ok((a, self.evaluate_node(cx, a)?)))
                    .collect::<Result<_, _>>()?;
                self.driver.enter_internal_call(f);
                let v = f.call_internal_function(internal::InternalCx {
                    func: *f,
                    driver: self.driver,
                    cache: self.cache,
                    rir: self.ir,
                    cx,
                    nid,
                    fn_call,
                    arg_values,
                })?;
                self.driver.exit_internal_call(f);
                Ok(v)
            }
            _ => Err(cx.err(
                fn_call.lparen_token,
                RunnerError::ExpectedType {
                    actual: v.rain_type_id(),
                    expected: &[RainTypeId::Function],
                },
            )),
        }
    }

    fn evaluate_binary_op(&mut self, cx: &mut Cx, op: &BinaryOp) -> ResultValue {
        let left = self.evaluate_node(cx, op.left)?;
        let left_type = left.rain_type_id();
        // Dot is a special case where we have to evaluate the right differently
        if op.op == BinaryOperatorKind::Dot {
            return self.evaluate_dot_operator(cx, op, &left, &left_type);
        }
        let right = self.evaluate_node(cx, op.right)?;
        let right_type = right.rain_type_id();

        match (left_type, op.op, right_type) {
            (RainTypeId::String, BinaryOperatorKind::Addition, RainTypeId::String) => {
                Self::perform_binary_op(cx, op, &left, &right, |left: &String, right: &String| {
                    Value::new(left.to_owned() + right)
                })
            }
            (RainTypeId::Integer, BinaryOperatorKind::Addition, RainTypeId::Integer) => {
                Self::perform_binary_op(
                    cx,
                    op,
                    &left,
                    &right,
                    |left: &RainInteger, right: &RainInteger| {
                        Value::new(RainInteger(&left.0 + &right.0))
                    },
                )
            }
            (RainTypeId::Integer, BinaryOperatorKind::Subtraction, RainTypeId::Integer) => {
                Self::perform_binary_op(
                    cx,
                    op,
                    &left,
                    &right,
                    |left: &RainInteger, right: &RainInteger| {
                        Value::new(RainInteger(&left.0 - &right.0))
                    },
                )
            }
            (RainTypeId::Integer, BinaryOperatorKind::Multiplication, RainTypeId::Integer) => {
                Self::perform_binary_op(
                    cx,
                    op,
                    &left,
                    &right,
                    |left: &RainInteger, right: &RainInteger| {
                        Value::new(RainInteger(&left.0 * &right.0))
                    },
                )
            }
            (RainTypeId::Integer, BinaryOperatorKind::Division, RainTypeId::Integer) => {
                Self::perform_binary_op(
                    cx,
                    op,
                    &left,
                    &right,
                    |left: &RainInteger, right: &RainInteger| {
                        Value::new(RainInteger(&left.0 / &right.0))
                    },
                )
            }
            (RainTypeId::Integer, BinaryOperatorKind::LogicalAnd, RainTypeId::Integer) => {
                Self::perform_binary_op(cx, op, &left, &right, |left: &bool, right: &bool| {
                    Value::new(*left && *right)
                })
            }
            (RainTypeId::Integer, BinaryOperatorKind::LogicalOr, RainTypeId::Integer) => {
                Self::perform_binary_op(cx, op, &left, &right, |left: &bool, right: &bool| {
                    Value::new(*left || *right)
                })
            }
            (RainTypeId::Integer, BinaryOperatorKind::Equals, RainTypeId::Integer) => {
                Self::perform_binary_op(
                    cx,
                    op,
                    &left,
                    &right,
                    |left: &RainInteger, right: &RainInteger| Value::new(left.0 == right.0),
                )
            }
            (RainTypeId::Integer, BinaryOperatorKind::NotEquals, RainTypeId::Integer) => {
                Self::perform_binary_op(
                    cx,
                    op,
                    &left,
                    &right,
                    |left: &RainInteger, right: &RainInteger| Value::new(left.0 != right.0),
                )
            }
            (RainTypeId::String, BinaryOperatorKind::Equals, RainTypeId::String) => {
                Self::perform_binary_op(cx, op, &left, &right, |left: &String, right: &String| {
                    Value::new(left == right)
                })
            }
            (RainTypeId::String, BinaryOperatorKind::NotEquals, RainTypeId::String) => {
                Self::perform_binary_op(cx, op, &left, &right, |left: &String, right: &String| {
                    Value::new(left != right)
                })
            }
            _ => Err(cx.err(
                op.op_span,
                RunnerError::Makeshift("binary op invalid for given types".into()),
            )),
        }
    }

    fn evaluate_dot_operator(
        &mut self,
        cx: &mut Cx,
        op: &BinaryOp,
        left: &Value,
        left_type: &RainTypeId,
    ) -> std::result::Result<Value, ErrorSpan<Throwing>> {
        match *left_type {
            RainTypeId::Module => {
                let Some(module_value) = left.downcast_ref::<Module>() else {
                    unreachable!()
                };
                match cx.module.get(op.right) {
                    Node::Ident(tls) => {
                        let name = tls.0.span.contents(&cx.module.src);
                        let Some(did) = self.ir.resolve_global_declaration(module_value.id, name)
                        else {
                            return Err(cx.err(tls.0.span, RunnerError::UnknownIdent));
                        };
                        self.evaluate_declaration(did)
                    }
                    _ => Err(cx.err(op.op_span, RunnerError::GenericRunError)),
                }
            }
            RainTypeId::Internal => match cx.module.get(op.right) {
                Node::Ident(tls) => {
                    let name = tls.0.span.contents(&cx.module.src);
                    InternalFunction::evaluate_internal_function_name(name)
                        .map(Value::new)
                        .ok_or_else(|| cx.err(tls.0.span, RunnerError::GenericRunError))
                }
                _ => Err(cx.err(op.op_span, RunnerError::GenericRunError)),
            },
            RainTypeId::Record => match cx.module.get(op.right) {
                Node::Ident(tls) => {
                    let Some(record_value) = left.downcast_ref::<RainRecord>() else {
                        unreachable!()
                    };
                    let name = tls.0.span.contents(&cx.module.src);
                    record_value.0.get(name).cloned().ok_or_else(|| {
                        cx.err(
                            tls.0.span,
                            RunnerError::RecordMissingEntry {
                                name: name.to_owned(),
                            },
                        )
                    })
                }
                _ => Err(cx.err(op.op_span, RunnerError::GenericRunError)),
            },
            _ => Err(cx.err(op.op_span, RunnerError::GenericRunError)),
        }
    }

    fn perform_binary_op<L, R, F>(
        cx: &mut Cx,
        op: &BinaryOp,
        left: &Value,
        right: &Value,
        f: F,
    ) -> ResultValue
    where
        L: ValueInner,
        R: ValueInner,
        F: FnOnce(&L, &R) -> Value,
    {
        let left = left
            .downcast_ref::<L>()
            .ok_or_else(|| cx.err(op.op_span, RunnerError::GenericRunError))?;
        let right = right
            .downcast_ref::<R>()
            .ok_or_else(|| cx.err(op.op_span, RunnerError::GenericRunError))?;
        Ok(f(left, right))
    }

    fn evaluate_if_condition(&mut self, cx: &mut Cx, if_condition: &IfCondition) -> ResultValue {
        let condition_value = self.evaluate_node(cx, if_condition.condition)?;
        let Some(condition_bool): Option<&bool> = condition_value.downcast_ref() else {
            return Err(cx.err(
                LocalSpan::default(),
                RunnerError::ExpectedType {
                    actual: condition_value.rain_type_id(),
                    expected: &[RainTypeId::Boolean],
                },
            ));
        };
        if *condition_bool {
            self.evaluate_node(cx, if_condition.then_block)
        } else {
            match if_condition.alternate {
                Some(AlternateCondition::IfElseCondition(if_condition)) => {
                    self.evaluate_node(cx, if_condition)
                }
                Some(AlternateCondition::ElseBlock(block)) => self.evaluate_node(cx, block),
                None => Ok(value_impl::get_unit()),
            }
        }
    }
}
