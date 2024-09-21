pub mod cache;
pub mod error;
pub mod value;

const MAX_CALL_DEPTH: usize = 500;

use std::{any::TypeId, collections::HashMap, sync::Arc};

use error::RunnerError;
use value::{
    RainFunction, RainInteger, RainInternal, RainInternalFunction, RainModule, RainTypeId,
    RainValue,
};

use crate::{
    ast::{AlternateCondition, BinaryOp, BinaryOperatorKind, FnCall, IfCondition, Node, NodeId},
    ir::{DeclarationId, IrModule, Rir},
    local_span::{ErrorLocalSpan, LocalSpan},
};

type ResultValue = Result<RainValue, ErrorLocalSpan<RunnerError>>;

struct Cx<'a> {
    module: &'a Arc<IrModule>,
    call_depth: usize,
    locals: HashMap<&'a str, RainValue>,
    args: HashMap<&'a str, RainValue>,
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
}

pub struct Runner {
    pub rir: Rir,
    pub cache: cache::Cache,
}

impl Runner {
    pub fn new(rir: Rir) -> Self {
        Self {
            rir,
            cache: cache::Cache::default(),
        }
    }

    pub fn evaluate_and_call(&mut self, id: DeclarationId) -> ResultValue {
        let v = self.evaluate_declaration(id)?;
        if v.any_type_id() == TypeId::of::<RainFunction>() {
            let Some(f) = v.downcast::<RainFunction>() else {
                unreachable!();
            };
            self.call_function(0, &f, vec![])
        } else {
            Ok(v)
        }
    }

    pub fn evaluate_declaration(&mut self, id: DeclarationId) -> ResultValue {
        let m = &Arc::clone(self.rir.get_module(id.module_id()));
        let nid = m.get_declaration(id.local_id());
        let node = m.get(nid);
        match node {
            Node::LetDeclare(let_declare) => self.evaluate_node(&mut Cx::new(m), let_declare.expr),
            Node::FnDeclare(_) => Ok(RainValue::new(RainFunction { id })),
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
                    self.evaluate_node(cx, *nid)?;
                }
                if let Some(nid) = block.statements.last() {
                    self.evaluate_node(cx, *nid)
                } else {
                    Ok(RainValue::new(()))
                }
            }
            Node::IfCondition(if_condition) => self.evaluate_if_condition(cx, if_condition),
            Node::FnCall(fn_call) => self.evaluate_fn_call(cx, fn_call),
            Node::Assignment(assignment) => {
                let v = self.evaluate_node(cx, assignment.expr)?;
                let name = assignment.name.span.contents(&cx.module.src);
                cx.locals.insert(name, v);
                Ok(RainValue::new(()))
            }
            Node::BinaryOp(binary_op) => self.evaluate_binary_op(cx, binary_op),
            Node::Ident(tls) => self
                .resolve_ident(cx, tls.span.contents(&cx.module.src))?
                .ok_or_else(|| tls.span.with_error(RunnerError::UnknownIdent)),
            Node::Internal(_) => Ok(RainValue::new(RainInternal)),
            Node::StringLiteral(lit) => match lit.prefix() {
                Some(crate::tokens::StringLiteralPrefix::Format) => todo!("format string"),
                None => Ok(RainValue::new(
                    lit.content_span().contents(&cx.module.src).to_owned(),
                )),
            },
            Node::IntegerLiteral(tls) => Ok(RainValue::new(
                tls.span
                    .contents(&cx.module.src)
                    .parse::<RainInteger>()
                    .map_err(|_| tls.span.with_error(RunnerError::InvalidIntegerLiteral))?,
            )),
            Node::TrueLiteral(_) => Ok(RainValue::new(true)),
            Node::FalseLiteral(_) => Ok(RainValue::new(false)),
        }
    }

    fn resolve_ident(
        &mut self,
        cx: &mut Cx,
        ident: &str,
    ) -> Result<Option<RainValue>, ErrorLocalSpan<RunnerError>> {
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

    fn evaluate_fn_call(&mut self, cx: &mut Cx, fn_call: &FnCall) -> ResultValue {
        let v = self.evaluate_node(cx, fn_call.callee)?;
        let v_type = v.rain_type_id();
        match v_type {
            RainTypeId::Function => {
                let Some(f) = v.downcast_ref::<RainFunction>() else {
                    unreachable!();
                };
                let arg_values: Vec<RainValue> = fn_call
                    .args
                    .iter()
                    .map(|a| self.evaluate_node(cx, *a))
                    .collect::<Result<_, _>>()?;
                if cx.call_depth >= MAX_CALL_DEPTH {
                    return Err(fn_call
                        .lparen_token
                        .span
                        .with_error(RunnerError::MaxCallDepth));
                }
                let key = self.cache.function_call_key(f, &arg_values);

                if let Some(v) = self.cache.get(&key) {
                    return Ok(v.clone());
                }
                let v = self.call_function(cx.call_depth + 1, f, arg_values)?;
                self.cache.put(key, v.clone());
                Ok(v)
            }
            RainTypeId::InternalFunction => {
                let Some(f) = v.downcast_ref::<RainInternalFunction>() else {
                    unreachable!()
                };
                let arg_values: Vec<RainValue> = fn_call
                    .args
                    .iter()
                    .map(|a| self.evaluate_node(cx, *a))
                    .collect::<Result<_, _>>()?;
                self.call_internal_function(cx, f, arg_values)
            }
            _ => Err(fn_call
                .lparen_token
                .span
                .with_error(RunnerError::ExpectedType(
                    v.rain_type_id(),
                    &[RainTypeId::Function],
                ))),
        }
    }

    fn call_function(
        &mut self,
        call_depth: usize,
        function: &RainFunction,
        arg_values: Vec<RainValue>,
    ) -> ResultValue {
        let m = &Arc::clone(self.rir.get_module(function.id.module_id()));
        let nid = m.get_declaration(function.id.local_id());
        let node = m.get(nid);
        match node {
            Node::FnDeclare(fn_declare) => {
                let args = fn_declare
                    .args
                    .iter()
                    .zip(arg_values)
                    .map(|(a, v)| (a.name.span.contents(&m.src), v))
                    .collect();
                let mut cx = Cx {
                    module: m,
                    call_depth,
                    args,
                    locals: HashMap::new(),
                };
                self.evaluate_node(&mut cx, fn_declare.block)
            }
            _ => unreachable!(),
        }
    }

    #[allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]
    fn call_internal_function(
        &mut self,
        cx: &mut Cx,
        function: &RainInternalFunction,
        arg_values: Vec<RainValue>,
    ) -> ResultValue {
        match function {
            RainInternalFunction::Print => {
                println!("{arg_values:?}");
                Ok(RainValue::new(()))
            }
            RainInternalFunction::Import => {
                let import_target = arg_values.first().unwrap();
                let import_path: &String = import_target.downcast_ref().unwrap();
                let resolved_path = cx
                    .module
                    .path
                    .as_ref()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join(import_path);
                let src = std::fs::read_to_string(&resolved_path).unwrap();
                let mut stream = crate::tokens::peek::PeekTokenStream::new(&src);
                let module = crate::ast::parser::parse_module(&mut stream).unwrap();
                let mid = self.rir.insert_module(Some(resolved_path), src, module);
                Ok(RainValue::new(RainModule { id: mid }))
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn evaluate_binary_op(&mut self, cx: &mut Cx, op: &BinaryOp) -> ResultValue {
        let left = self.evaluate_node(cx, op.left)?;
        match op.op {
            BinaryOperatorKind::Addition => Ok(RainValue::new(RainInteger(
                &left
                    .downcast_ref::<RainInteger>()
                    .ok_or_else(|| op.op_span.with_error(RunnerError::GenericTypeError))?
                    .0
                    + &self
                        .evaluate_node(cx, op.right)?
                        .downcast_ref::<RainInteger>()
                        .ok_or_else(|| op.op_span.with_error(RunnerError::GenericTypeError))?
                        .0,
            ))),
            BinaryOperatorKind::Subtraction => Ok(RainValue::new(RainInteger(
                &left
                    .downcast_ref::<RainInteger>()
                    .ok_or_else(|| op.op_span.with_error(RunnerError::GenericTypeError))?
                    .0
                    - &self
                        .evaluate_node(cx, op.right)?
                        .downcast_ref::<RainInteger>()
                        .ok_or_else(|| op.op_span.with_error(RunnerError::GenericTypeError))?
                        .0,
            ))),
            BinaryOperatorKind::Multiplication => Ok(RainValue::new(RainInteger(
                &left
                    .downcast_ref::<RainInteger>()
                    .ok_or_else(|| op.op_span.with_error(RunnerError::GenericTypeError))?
                    .0
                    * &self
                        .evaluate_node(cx, op.right)?
                        .downcast_ref::<RainInteger>()
                        .ok_or_else(|| op.op_span.with_error(RunnerError::GenericTypeError))?
                        .0,
            ))),
            BinaryOperatorKind::Division => Ok(RainValue::new(RainInteger(
                &left
                    .downcast_ref::<RainInteger>()
                    .ok_or_else(|| op.op_span.with_error(RunnerError::GenericTypeError))?
                    .0
                    / &self
                        .evaluate_node(cx, op.right)?
                        .downcast_ref::<RainInteger>()
                        .ok_or_else(|| op.op_span.with_error(RunnerError::GenericTypeError))?
                        .0,
            ))),
            BinaryOperatorKind::LogicalAnd => Ok(RainValue::new(
                *left
                    .downcast_ref::<bool>()
                    .ok_or_else(|| op.op_span.with_error(RunnerError::GenericTypeError))?
                    && *self
                        .evaluate_node(cx, op.right)?
                        .downcast_ref::<bool>()
                        .ok_or_else(|| op.op_span.with_error(RunnerError::GenericTypeError))?,
            )),
            BinaryOperatorKind::LogicalOr => Ok(RainValue::new(
                *left
                    .downcast_ref::<bool>()
                    .ok_or_else(|| op.op_span.with_error(RunnerError::GenericTypeError))?
                    || *self
                        .evaluate_node(cx, op.right)?
                        .downcast_ref::<bool>()
                        .ok_or_else(|| op.op_span.with_error(RunnerError::GenericTypeError))?,
            )),
            BinaryOperatorKind::Equals => Ok(RainValue::new(
                left.downcast_ref::<RainInteger>()
                    .ok_or_else(|| op.op_span.with_error(RunnerError::GenericTypeError))?
                    .0
                    == self
                        .evaluate_node(cx, op.right)?
                        .downcast_ref::<RainInteger>()
                        .ok_or_else(|| op.op_span.with_error(RunnerError::GenericTypeError))?
                        .0,
            )),
            BinaryOperatorKind::NotEquals => todo!("evaluate not equality"),
            BinaryOperatorKind::Dot => match left.rain_type_id() {
                RainTypeId::Module => {
                    let Some(module_value) = left.downcast_ref::<RainModule>() else {
                        unreachable!()
                    };
                    match cx.module.get(op.right) {
                        Node::Ident(tls) => {
                            let name = tls.span.contents(&cx.module.src);
                            let Some(did) =
                                self.rir.resolve_global_declaration(module_value.id, name)
                            else {
                                return Err(tls.span.with_error(RunnerError::UnknownIdent));
                            };
                            self.evaluate_declaration(did)
                        }
                        _ => Err(op.op_span.with_error(RunnerError::GenericTypeError)),
                    }
                }
                RainTypeId::Internal => match cx.module.get(op.right) {
                    Node::Ident(tls) => {
                        let name = tls.span.contents(&cx.module.src);
                        match name {
                            "print" => Ok(RainValue::new(RainInternalFunction::Print)),
                            "import" => Ok(RainValue::new(RainInternalFunction::Import)),
                            _ => Err(tls.span.with_error(RunnerError::GenericTypeError)),
                        }
                    }
                    _ => Err(op.op_span.with_error(RunnerError::GenericTypeError)),
                },
                _ => Err(op.op_span.with_error(RunnerError::GenericTypeError)),
            },
        }
    }

    fn evaluate_if_condition(&mut self, cx: &mut Cx, if_condition: &IfCondition) -> ResultValue {
        let condition_value = self.evaluate_node(cx, if_condition.condition)?;
        let Some(condition_bool): Option<&bool> = condition_value.downcast_ref() else {
            return Err(LocalSpan::default().with_error(RunnerError::ExpectedType(
                condition_value.rain_type_id(),
                &[RainTypeId::Boolean],
            )));
        };
        if *condition_bool {
            self.evaluate_node(cx, if_condition.then_block)
        } else {
            match if_condition.alternate {
                Some(AlternateCondition::IfElseCondition(if_condition)) => {
                    self.evaluate_node(cx, if_condition)
                }
                Some(AlternateCondition::ElseBlock(block)) => self.evaluate_node(cx, block),
                None => Ok(RainValue::new(())),
            }
        }
    }
}
