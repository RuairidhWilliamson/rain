pub mod cache;
pub mod dep;
pub mod error;
pub mod internal;
pub mod value;

use std::{collections::HashMap, sync::Arc, time::Instant};

use cache::CacheEntry;
use error::{RunnerError, Throwing};
use indexmap::IndexMap;
use internal::InternalFunction;
use value::{RainInteger, RainList, RainRecord, RainTypeId, Value};

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
    deps: Vec<dep::Dep>,
}

impl<'a> Cx<'a> {
    fn new(module: &'a Arc<IrModule>) -> Self {
        Self {
            module,
            call_depth: 0,
            args: HashMap::new(),
            locals: HashMap::new(),
            deps: Vec::new(),
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
    pub cache: &'a dyn cache::CacheTrait,
    pub driver: &'a D,
}

impl<'a, D: DriverTrait> Runner<'a, D> {
    pub fn new(rir: &'a mut Rir, cache: &'a dyn cache::CacheTrait, driver: &'a D) -> Self {
        Self {
            ir: rir,
            cache,
            driver,
        }
    }

    pub fn evaluate_and_call(&mut self, id: DeclarationId, args: &[String]) -> ResultValue {
        let v = self.evaluate_declaration(id)?;
        let Value::Function(f) = v else {
            return Ok(v);
        };
        let m = &Arc::clone(self.ir.get_module(f.module_id()));
        let nid = m.get_declaration(f.local_id());
        let node = m.get(nid);
        match node {
            Node::FnDeclare(fn_declare) => {
                if fn_declare.args.len() != args.len() {
                    return Err(fn_declare.rparen_token.span.with_module(m.id).with_error(
                        RunnerError::IncorrectArgs {
                            required: fn_declare.args.len()..=fn_declare.args.len(),
                            actual: args.len(),
                        }
                        .into(),
                    ));
                }
                let args = fn_declare
                    .args
                    .iter()
                    .zip(args)
                    .map(|(a, v)| {
                        (
                            a.name.span.contents(&m.src),
                            Value::String(Arc::new(v.clone())),
                        )
                    })
                    .collect();
                let mut cx = Cx {
                    module: m,
                    call_depth: 0,
                    args,
                    locals: HashMap::new(),
                    deps: Vec::new(),
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
            Node::FnDeclare(_) => Ok(Value::Function(id)),
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
                Ok(prev.unwrap_or(Value::Unit))
            }
            Node::IfCondition(if_condition) => self.evaluate_if_condition(cx, if_condition),
            Node::FnCall(fn_call) => self.evaluate_fn_call(cx, nid, fn_call),
            Node::Assignment(assignment) => {
                let v = self.evaluate_node(cx, assignment.expr)?;
                let name = assignment.name.span.contents(&cx.module.src);
                cx.locals.insert(name, v);
                Ok(Value::Unit)
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
            Node::InternalLiteral(_) => Ok(Value::Internal),
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
                    Ok(Value::String(Arc::new(contents.to_string())))
                }
            },
            Node::IntegerLiteral(tls) => Ok(Value::Integer(Arc::new(RainInteger(
                tls.0
                    .span
                    .contents(&cx.module.src)
                    .parse::<num_bigint::BigInt>()
                    .map_err(|_| cx.err(tls.0, RunnerError::InvalidIntegerLiteral))?,
            )))),
            Node::TrueLiteral(_) => Ok(Value::Boolean(true)),
            Node::FalseLiteral(_) => Ok(Value::Boolean(false)),
            Node::Record(record) => {
                let mut builder = IndexMap::new();
                for e in &record.fields {
                    builder.insert(
                        e.key.span.contents(&cx.module.src).to_owned(),
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
        match &v {
            Value::Function(f) => {
                if cx.call_depth >= MAX_CALL_DEPTH {
                    return Err(cx.err(fn_call.lparen_token, RunnerError::MaxCallDepth));
                }
                let arg_values: Vec<Value> = fn_call
                    .args
                    .iter()
                    .map(|a| self.evaluate_node(cx, *a))
                    .collect::<Result<_, _>>()?;
                let key = cache::CacheKey::Declaration {
                    declaration: *f,
                    args: arg_values.clone(),
                };

                if let Some(cache_entry) = self.cache.get(&key) {
                    cx.deps.extend(cache_entry.deps);
                    return Ok(cache_entry.value);
                }
                let start = Instant::now();
                let m = &Arc::clone(self.ir.get_module(f.module_id()));
                let nid = m.get_declaration(f.local_id());
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
                let mut callee_cx = Cx {
                    module: m,
                    call_depth: cx.call_depth + 1,
                    args,
                    locals: HashMap::new(),
                    deps: Vec::new(),
                };
                let result = self.evaluate_node(&mut callee_cx, fn_declare.block)?;
                self.driver.exit_call(function_name);
                self.cache.put(
                    key,
                    CacheEntry {
                        execution_time: start.elapsed(),
                        expires: None,
                        etag: None,
                        deps: callee_cx.deps.clone(),
                        value: result.clone(),
                    },
                );
                cx.deps.extend(callee_cx.deps);
                Ok(result)
            }
            Value::InternalFunction(f) => {
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
        // Dot is a special case where we have to evaluate the right differently
        if op.op == BinaryOperatorKind::Dot {
            return self.evaluate_dot_operator(cx, op, &left);
        }
        let right = self.evaluate_node(cx, op.right)?;

        match (left, op.op, right) {
            (Value::String(left), BinaryOperatorKind::Addition, Value::String(right)) => {
                Ok(Value::String(Arc::new(left.to_string() + &**right)))
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
    ) -> std::result::Result<Value, ErrorSpan<Throwing>> {
        match left {
            Value::Module(module_value) => match cx.module.get(op.right) {
                Node::Ident(tls) => {
                    let name = tls.0.span.contents(&cx.module.src);
                    let Some(did) = self.ir.resolve_global_declaration(*module_value, name) else {
                        return Err(cx.err(tls.0.span, RunnerError::UnknownIdent));
                    };
                    self.evaluate_declaration(did)
                }
                _ => Err(cx.err(
                    op.op_span,
                    RunnerError::Makeshift("dot operator right side is not ident".into()),
                )),
            },
            Value::Internal => match cx.module.get(op.right) {
                Node::Ident(tls) => {
                    let name = tls.0.span.contents(&cx.module.src);
                    InternalFunction::evaluate_internal_function_name(name)
                        .map(Value::InternalFunction)
                        .ok_or_else(|| {
                            cx.err(
                                tls.0.span,
                                RunnerError::Makeshift("unknown internal function name".into()),
                            )
                        })
                }
                _ => Err(cx.err(
                    op.op_span,
                    RunnerError::Makeshift("dot operator right side is not ident".into()),
                )),
            },
            Value::Record(record_value) => match cx.module.get(op.right) {
                Node::Ident(tls) => {
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
                _ => Err(cx.err(
                    op.op_span,
                    RunnerError::Makeshift("dot operator right side is not ident".into()),
                )),
            },
            _ => Err(cx.err(
                op.op_span,
                RunnerError::ExpectedType {
                    actual: left.rain_type_id(),
                    expected: &[RainTypeId::Module, RainTypeId::Internal, RainTypeId::Record],
                },
            )),
        }
    }

    fn evaluate_if_condition(&mut self, cx: &mut Cx, if_condition: &IfCondition) -> ResultValue {
        let condition_value = self.evaluate_node(cx, if_condition.condition)?;
        let Value::Boolean(condition_bool) = condition_value else {
            return Err(cx.err(
                LocalSpan::default(),
                RunnerError::ExpectedType {
                    actual: condition_value.rain_type_id(),
                    expected: &[RainTypeId::Boolean],
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
}
