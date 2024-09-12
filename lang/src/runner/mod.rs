pub mod cache;
pub mod error;
pub mod value;

const MAX_CALL_DEPTH: usize = 500;

use std::{any::TypeId, collections::HashMap};

use error::RunnerError;
use value::{
    RainFunction, RainInteger, RainInternal, RainInternalFunction, RainModule, RainTypeId,
    RainValue,
};

use crate::{
    ast::{
        binary_op::{BinaryOp, BinaryOperatorKind},
        display::AstDisplay,
        expr::{AlternateCondition, Expr, FnCall, IfCondition},
        Block, LetDeclare,
    },
    error::ErrorSpan,
    ir::{DeclarationId, Module, Rir},
};

type ResultValue = Result<RainValue, ErrorSpan<RunnerError>>;

struct Cx<'a> {
    module: &'a Module,
    call_depth: usize,
    locals: HashMap<&'a str, RainValue>,
    args: HashMap<&'a str, RainValue>,
}

impl<'a> Cx<'a> {
    fn new(module: &'a Module) -> Self {
        Self {
            module,
            call_depth: 0,
            args: HashMap::new(),
            locals: HashMap::new(),
        }
    }
}

pub struct Runner<'a> {
    rir: &'a Rir,
    cache: cache::Cache,
}

impl<'a> Runner<'a> {
    pub fn new(rir: &'a Rir) -> Self {
        Self {
            rir,
            cache: cache::Cache::default(),
        }
    }

    pub fn evaluate_and_call(&mut self, id: DeclarationId) -> ResultValue {
        let v = self.evaluate(id)?;
        if v.any_type_id() == TypeId::of::<RainFunction>() {
            let Some(f) = v.downcast::<RainFunction>() else {
                unreachable!();
            };
            self.call_function(0, &f, vec![])
        } else {
            Ok(v)
        }
    }

    pub fn evaluate(&mut self, id: DeclarationId) -> ResultValue {
        let m = self.rir.get_module(id.module_id());
        let d = m.get_declaration(id.local_id());
        match d {
            crate::ast::Declaration::LetDeclare(LetDeclare { expr, .. }) => {
                self.evaluate_expr(&mut Cx::new(m), expr)
            }
            crate::ast::Declaration::FnDeclare(_) => Ok(RainValue::new(RainFunction { id })),
        }
    }

    fn evaluate_block(&mut self, cx: &mut Cx, block: &Block) -> ResultValue {
        for s in &block.statements[..block.statements.len() - 1] {
            match s {
                crate::ast::Statement::Expr(expr) => {
                    self.evaluate_expr(cx, expr)?;
                }
                crate::ast::Statement::Assignment(assign) => {
                    let v = self.evaluate_expr(cx, &assign.expr)?;
                    let name = assign.name.span.contents(&cx.module.src);
                    cx.locals.insert(name, v);
                }
            }
        }
        if let Some(s) = block.statements.last() {
            match s {
                crate::ast::Statement::Expr(expr) => self.evaluate_expr(cx, expr),
                crate::ast::Statement::Assignment(assign) => {
                    let v = self.evaluate_expr(cx, &assign.expr)?;
                    let name = assign.name.span.contents(&cx.module.src);
                    cx.locals.insert(name, v);
                    Ok(RainValue::new(()))
                }
            }
        } else {
            Ok(RainValue::new(()))
        }
    }

    fn resolve_ident(
        &mut self,
        cx: &mut Cx,
        ident: &str,
    ) -> Result<Option<RainValue>, ErrorSpan<RunnerError>> {
        if let Some(v) = cx.locals.get(ident) {
            return Ok(Some(v.clone()));
        }
        if let Some(v) = cx.args.get(ident) {
            return Ok(Some(v.clone()));
        }
        let Some(declaration_id) = self.rir.resolve_global_declaration(cx.module.id, ident) else {
            return Ok(None);
        };
        Ok(Some(self.evaluate(declaration_id)?))
    }

    fn evaluate_expr(&mut self, cx: &mut Cx, expr: &Expr) -> ResultValue {
        match expr {
            Expr::Ident(tls) => {
                let ident_name = tls.span.contents(&cx.module.src);
                self.resolve_ident(cx, ident_name)?
                    .ok_or_else(|| tls.span.with_error(RunnerError::UnknownIdent))
            }
            Expr::StringLiteral(lit) => match lit.prefix() {
                Some(crate::tokens::StringLiteralPrefix::Format) => todo!("format string"),
                None => Ok(RainValue::new(
                    lit.content_span().contents(&cx.module.src).to_owned(),
                )),
            },
            Expr::IntegerLiteral(tls) => Ok(RainValue::new(
                tls.span
                    .contents(&cx.module.src)
                    .parse::<RainInteger>()
                    .map_err(|_| tls.span.with_error(RunnerError::InvalidIntegerLiteral))?,
            )),
            Expr::TrueLiteral(_) => Ok(RainValue::new(true)),
            Expr::FalseLiteral(_) => Ok(RainValue::new(false)),
            Expr::BinaryOp(b) => self.evaluate_binary_op(cx, b),
            Expr::FnCall(fn_call) => self.evaluate_fn_call(cx, fn_call),
            Expr::If(if_condition) => self.evaluate_if_condition(cx, if_condition),
            Expr::Internal(_) => Ok(RainValue::new(RainInternal)),
        }
    }

    fn evaluate_fn_call(&mut self, cx: &mut Cx, fn_call: &FnCall) -> ResultValue {
        let v = self.evaluate_expr(cx, &fn_call.callee)?;
        let v_type = v.rain_type_id();
        match v_type {
            RainTypeId::Function => {
                let Some(f) = v.downcast_ref::<RainFunction>() else {
                    unreachable!();
                };
                let arg_values: Vec<RainValue> = fn_call
                    .args
                    .args
                    .iter()
                    .map(|a| self.evaluate_expr(cx, a))
                    .collect::<Result<_, _>>()?;
                if cx.call_depth >= MAX_CALL_DEPTH {
                    return Err(fn_call.span().with_error(RunnerError::MaxCallDepth));
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
                    .args
                    .iter()
                    .map(|a| self.evaluate_expr(cx, a))
                    .collect::<Result<_, _>>()?;
                self.call_internal_function(f, arg_values)
            }
            _ => Err(fn_call.callee.span().with_error(RunnerError::ExpectedType(
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
        let m = self.rir.get_module(function.id.module_id());
        let d = m.get_declaration(function.id.local_id());
        match d {
            crate::ast::Declaration::FnDeclare(fn_declare) => {
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
                self.evaluate_block(&mut cx, &fn_declare.block)
            }
            crate::ast::Declaration::LetDeclare(_) => unreachable!(),
        }
    }

    fn call_internal_function(
        &mut self,
        function: &RainInternalFunction,
        _arg_values: Vec<RainValue>,
    ) -> ResultValue {
        match function {
            RainInternalFunction::Print => todo!("implement internal.print"),
            RainInternalFunction::Import => todo!("implement internal.import"),
        }
    }

    fn evaluate_binary_op(&mut self, cx: &mut Cx, op: &BinaryOp) -> ResultValue {
        let left = self.evaluate_expr(cx, &op.left)?;
        match op.op.kind {
            BinaryOperatorKind::Addition => Ok(RainValue::new(RainInteger(
                &left
                    .downcast_ref::<RainInteger>()
                    .ok_or_else(|| op.op.span.with_error(RunnerError::GenericTypeError))?
                    .0
                    + &self
                        .evaluate_expr(cx, &op.right)?
                        .downcast_ref::<RainInteger>()
                        .ok_or_else(|| op.op.span.with_error(RunnerError::GenericTypeError))?
                        .0,
            ))),
            BinaryOperatorKind::Subtraction => Ok(RainValue::new(RainInteger(
                &left
                    .downcast_ref::<RainInteger>()
                    .ok_or_else(|| op.op.span.with_error(RunnerError::GenericTypeError))?
                    .0
                    - &self
                        .evaluate_expr(cx, &op.right)?
                        .downcast_ref::<RainInteger>()
                        .ok_or_else(|| op.op.span.with_error(RunnerError::GenericTypeError))?
                        .0,
            ))),
            BinaryOperatorKind::Multiplication => Ok(RainValue::new(RainInteger(
                &left
                    .downcast_ref::<RainInteger>()
                    .ok_or_else(|| op.op.span.with_error(RunnerError::GenericTypeError))?
                    .0
                    * &self
                        .evaluate_expr(cx, &op.right)?
                        .downcast_ref::<RainInteger>()
                        .ok_or_else(|| op.op.span.with_error(RunnerError::GenericTypeError))?
                        .0,
            ))),
            BinaryOperatorKind::Division => Ok(RainValue::new(RainInteger(
                &left
                    .downcast_ref::<RainInteger>()
                    .ok_or_else(|| op.op.span.with_error(RunnerError::GenericTypeError))?
                    .0
                    / &self
                        .evaluate_expr(cx, &op.right)?
                        .downcast_ref::<RainInteger>()
                        .ok_or_else(|| op.op.span.with_error(RunnerError::GenericTypeError))?
                        .0,
            ))),
            BinaryOperatorKind::LogicalAnd => Ok(RainValue::new(
                *left
                    .downcast_ref::<bool>()
                    .ok_or_else(|| op.op.span.with_error(RunnerError::GenericTypeError))?
                    && *self
                        .evaluate_expr(cx, &op.right)?
                        .downcast_ref::<bool>()
                        .ok_or_else(|| op.op.span.with_error(RunnerError::GenericTypeError))?,
            )),
            BinaryOperatorKind::LogicalOr => Ok(RainValue::new(
                *left
                    .downcast_ref::<bool>()
                    .ok_or_else(|| op.op.span.with_error(RunnerError::GenericTypeError))?
                    || *self
                        .evaluate_expr(cx, &op.right)?
                        .downcast_ref::<bool>()
                        .ok_or_else(|| op.op.span.with_error(RunnerError::GenericTypeError))?,
            )),
            BinaryOperatorKind::Equals => Ok(RainValue::new(
                left.downcast_ref::<RainInteger>()
                    .ok_or_else(|| op.op.span.with_error(RunnerError::GenericTypeError))?
                    .0
                    == self
                        .evaluate_expr(cx, &op.right)?
                        .downcast_ref::<RainInteger>()
                        .ok_or_else(|| op.op.span.with_error(RunnerError::GenericTypeError))?
                        .0,
            )),
            BinaryOperatorKind::NotEquals => todo!("evaluate not equality"),
            BinaryOperatorKind::Dot => match left.rain_type_id() {
                RainTypeId::Module => {
                    let Some(module_value) = left.downcast_ref::<RainModule>() else {
                        unreachable!()
                    };
                    let _module = self.rir.get_module(module_value.id);
                    todo!("implement module")
                }
                RainTypeId::Internal => match op.right.as_ref() {
                    Expr::Ident(tls) => {
                        let name = tls.span.contents(&cx.module.src);
                        match name {
                            "print" => Ok(RainValue::new(RainInternalFunction::Print)),
                            "import" => Ok(RainValue::new(RainInternalFunction::Import)),
                            _ => Err(tls.span.with_error(RunnerError::GenericTypeError)),
                        }
                    }
                    _ => Err(op.right.span().with_error(RunnerError::GenericTypeError)),
                },
                _ => Err(op.op.span.with_error(RunnerError::GenericTypeError)),
            },
        }
    }

    fn evaluate_if_condition(
        &mut self,
        cx: &mut Cx,
        IfCondition {
            condition,
            then,
            alternate,
        }: &IfCondition,
    ) -> ResultValue {
        let condition_value = self.evaluate_expr(cx, condition)?;
        let Some(condition_bool): Option<&bool> = condition_value.downcast_ref() else {
            return Err(condition.span().with_error(RunnerError::ExpectedType(
                condition_value.rain_type_id(),
                &[RainTypeId::Boolean],
            )));
        };
        if *condition_bool {
            self.evaluate_block(cx, then)
        } else {
            match alternate {
                Some(AlternateCondition::IfElse(if_condition)) => {
                    self.evaluate_if_condition(cx, if_condition)
                }
                Some(AlternateCondition::Else(block)) => self.evaluate_block(cx, block),
                None => Ok(RainValue::new(())),
            }
        }
    }
}
