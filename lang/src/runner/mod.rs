pub mod cache;
pub mod error;
pub mod hash;
mod internal;
pub mod value;
pub mod value_impl;

use std::{collections::HashMap, num::NonZeroUsize, sync::Arc, time::Instant};

use error::RunnerError;
use internal::InternalFunction;
use value::{RainTypeId, Value};
use value_impl::{Module, RainFunction, RainInteger, RainInternal, RainUnit};

use crate::{
    afs::file_system::FileSystem,
    ast::{AlternateCondition, BinaryOp, BinaryOperatorKind, FnCall, IfCondition, Node, NodeId},
    ir::{DeclarationId, IrModule, Rir},
    local_span::LocalSpan,
    span::ErrorSpan,
};

const MAX_CALL_DEPTH: usize = 500;
#[expect(unsafe_code)]
const CACHE_SIZE: NonZeroUsize = unsafe { NonZeroUsize::new_unchecked(1024) };

type ResultValue = Result<Value, ErrorSpan<RunnerError>>;

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

    fn err(&self, s: impl Into<LocalSpan>, err: RunnerError) -> ErrorSpan<RunnerError> {
        s.into().with_module(self.module.id).with_error(err)
    }

    fn nid_err(&self, nid: NodeId, err: RunnerError) -> ErrorSpan<RunnerError> {
        self.module
            .span(nid)
            .with_module(self.module.id)
            .with_error(err)
    }
}

pub struct Runner {
    pub rir: Rir,
    pub cache: cache::Cache,
    pub file_system: Box<dyn FileSystem>,
}

impl Runner {
    pub fn new(rir: Rir, file_system: Box<dyn FileSystem>) -> Self {
        Self {
            rir,
            cache: cache::Cache::new(CACHE_SIZE),
            file_system,
        }
    }

    pub fn evaluate_and_call(&mut self, id: DeclarationId) -> ResultValue {
        let v = self.evaluate_declaration(id)?;
        let Some(f) = v.downcast_ref::<RainFunction>() else {
            return Ok(v);
        };
        let m = &Arc::clone(self.rir.get_module(f.id.module_id()));
        let nid = m.get_declaration(f.id.local_id());
        let node = m.get(nid);
        match node {
            Node::FnDeclare(fn_declare) => {
                if !fn_declare.args.is_empty() {
                    return Err(fn_declare.rparen_token.span.with_module(m.id).with_error(
                        RunnerError::IncorrectArgs {
                            required: fn_declare.args.len()..=fn_declare.args.len(),
                            actual: 0,
                        },
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
        let m = &Arc::clone(self.rir.get_module(id.module_id()));
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
                for nid in &block.statements[..block.statements.len() - 1] {
                    let v = self.evaluate_node(cx, *nid)?;
                    // Shortcut errors in block
                    if v.rain_type_id() == RainTypeId::Error {
                        return Ok(v);
                    }
                }
                if let Some(nid) = block.statements.last() {
                    self.evaluate_node(cx, *nid)
                } else {
                    Ok(Value::new(RainUnit))
                }
            }
            Node::IfCondition(if_condition) => self.evaluate_if_condition(cx, if_condition),
            Node::FnCall(fn_call) => self.evaluate_fn_call(cx, nid, fn_call),
            Node::Assignment(assignment) => {
                let v = self.evaluate_node(cx, assignment.expr)?;
                let name = assignment.name.span.contents(&cx.module.src);
                cx.locals.insert(name, v);
                Ok(Value::new(RainUnit))
            }
            Node::BinaryOp(binary_op) => self.evaluate_binary_op(cx, binary_op),
            Node::Ident(tls) => self
                .resolve_ident(cx, tls.0.span.contents(&cx.module.src))?
                .ok_or_else(|| {
                    tls.0
                        .span
                        .with_module(cx.module.id)
                        .with_error(RunnerError::UnknownIdent)
                }),
            Node::InternalLiteral(_) => Ok(Value::new(RainInternal)),
            Node::StringLiteral(lit) => match lit.prefix() {
                Some(crate::tokens::StringLiteralPrefix::Format) => todo!("format string"),
                None => Ok(Value::new(
                    lit.content_span().contents(&cx.module.src).to_owned(),
                )),
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
        }
    }

    fn resolve_ident(
        &mut self,
        cx: &mut Cx,
        ident: &str,
    ) -> Result<Option<Value>, ErrorSpan<RunnerError>> {
        if let Some(v) = cx.locals.get(ident) {
            return Ok(Some(v.clone()));
        }
        if let Some(v) = cx.args.get(ident) {
            return Ok(Some(v.clone()));
        }
        let Some(declaration_id) = self.rir.resolve_global_declaration(cx.module.id, ident) else {
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
                let key = self.cache.function_key(f, arg_values.clone());

                if let Some(v) = self.cache.get_value(&key) {
                    return Ok(v.clone());
                }
                let start = Instant::now();
                let m = &Arc::clone(self.rir.get_module(f.id.module_id()));
                let nid = m.get_declaration(f.id.local_id());
                let node = m.get(nid);
                let Node::FnDeclare(fn_declare) = node else {
                    unreachable!();
                };
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
                self.cache.put(key, start.elapsed(), result.clone());
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
                let key = self
                    .cache
                    .function_key(*f, arg_values.iter().map(|(_, a)| a.clone()).collect());
                if let Some(v) = self.cache.get_value(&key) {
                    return Ok(v.clone());
                }
                let start = Instant::now();
                let v = f.call_internal_function(
                    self.file_system.as_ref(),
                    &mut self.rir,
                    cx,
                    nid,
                    fn_call,
                    arg_values,
                )?;
                self.cache.put(key, start.elapsed(), v.clone());
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

    #[expect(clippy::too_many_lines)]
    fn evaluate_binary_op(&mut self, cx: &mut Cx, op: &BinaryOp) -> ResultValue {
        let left = self.evaluate_node(cx, op.left)?;
        match op.op {
            BinaryOperatorKind::Addition => Ok(Value::new(RainInteger(
                &left
                    .downcast_ref::<RainInteger>()
                    .ok_or_else(|| cx.err(op.op_span, RunnerError::GenericTypeError))?
                    .0
                    + &self
                        .evaluate_node(cx, op.right)?
                        .downcast_ref::<RainInteger>()
                        .ok_or_else(|| cx.err(op.op_span, RunnerError::GenericTypeError))?
                        .0,
            ))),
            BinaryOperatorKind::Subtraction => Ok(Value::new(RainInteger(
                &left
                    .downcast_ref::<RainInteger>()
                    .ok_or_else(|| cx.err(op.op_span, RunnerError::GenericTypeError))?
                    .0
                    - &self
                        .evaluate_node(cx, op.right)?
                        .downcast_ref::<RainInteger>()
                        .ok_or_else(|| cx.err(op.op_span, RunnerError::GenericTypeError))?
                        .0,
            ))),
            BinaryOperatorKind::Multiplication => Ok(Value::new(RainInteger(
                &left
                    .downcast_ref::<RainInteger>()
                    .ok_or_else(|| cx.err(op.op_span, RunnerError::GenericTypeError))?
                    .0
                    * &self
                        .evaluate_node(cx, op.right)?
                        .downcast_ref::<RainInteger>()
                        .ok_or_else(|| cx.err(op.op_span, RunnerError::GenericTypeError))?
                        .0,
            ))),
            BinaryOperatorKind::Division => Ok(Value::new(RainInteger(
                &left
                    .downcast_ref::<RainInteger>()
                    .ok_or_else(|| cx.err(op.op_span, RunnerError::GenericTypeError))?
                    .0
                    / &self
                        .evaluate_node(cx, op.right)?
                        .downcast_ref::<RainInteger>()
                        .ok_or_else(|| cx.err(op.op_span, RunnerError::GenericTypeError))?
                        .0,
            ))),
            BinaryOperatorKind::LogicalAnd => Ok(Value::new(
                *left
                    .downcast_ref::<bool>()
                    .ok_or_else(|| cx.err(op.op_span, RunnerError::GenericTypeError))?
                    && *self
                        .evaluate_node(cx, op.right)?
                        .downcast_ref::<bool>()
                        .ok_or_else(|| cx.err(op.op_span, RunnerError::GenericTypeError))?,
            )),
            BinaryOperatorKind::LogicalOr => Ok(Value::new(
                *left
                    .downcast_ref::<bool>()
                    .ok_or_else(|| cx.err(op.op_span, RunnerError::GenericTypeError))?
                    || *self
                        .evaluate_node(cx, op.right)?
                        .downcast_ref::<bool>()
                        .ok_or_else(|| cx.err(op.op_span, RunnerError::GenericTypeError))?,
            )),
            BinaryOperatorKind::Equals => Ok(Value::new(
                left.downcast_ref::<RainInteger>()
                    .ok_or_else(|| cx.err(op.op_span, RunnerError::GenericTypeError))?
                    .0
                    == self
                        .evaluate_node(cx, op.right)?
                        .downcast_ref::<RainInteger>()
                        .ok_or_else(|| cx.err(op.op_span, RunnerError::GenericTypeError))?
                        .0,
            )),
            BinaryOperatorKind::NotEquals => Ok(Value::new(
                left.downcast_ref::<RainInteger>()
                    .ok_or_else(|| cx.err(op.op_span, RunnerError::GenericTypeError))?
                    .0
                    != self
                        .evaluate_node(cx, op.right)?
                        .downcast_ref::<RainInteger>()
                        .ok_or_else(|| cx.err(op.op_span, RunnerError::GenericTypeError))?
                        .0,
            )),
            BinaryOperatorKind::Dot => match left.rain_type_id() {
                RainTypeId::Module => {
                    let Some(module_value) = left.downcast_ref::<Module>() else {
                        unreachable!()
                    };
                    match cx.module.get(op.right) {
                        Node::Ident(tls) => {
                            let name = tls.0.span.contents(&cx.module.src);
                            let Some(did) =
                                self.rir.resolve_global_declaration(module_value.id, name)
                            else {
                                return Err(cx.err(tls.0.span, RunnerError::UnknownIdent));
                            };
                            self.evaluate_declaration(did)
                        }
                        _ => Err(cx.err(op.op_span, RunnerError::GenericTypeError)),
                    }
                }
                RainTypeId::Internal => match cx.module.get(op.right) {
                    Node::Ident(tls) => {
                        let name = tls.0.span.contents(&cx.module.src);
                        InternalFunction::evaluate_internal_function_name(name)
                            .map(Value::new)
                            .ok_or_else(|| cx.err(tls.0.span, RunnerError::GenericTypeError))
                    }
                    _ => Err(cx.err(op.op_span, RunnerError::GenericTypeError)),
                },
                _ => Err(cx.err(op.op_span, RunnerError::GenericTypeError)),
            },
        }
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
                None => Ok(Value::new(RainUnit)),
            }
        }
    }
}
