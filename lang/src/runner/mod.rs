pub mod cache;
pub mod error;
pub mod value;

const MAX_CALL_DEPTH: usize = 500;

use std::{any::TypeId, collections::HashMap, sync::Arc};

use error::RunnerError;
use value::{RainFunction, RainInteger, RainValue};

use crate::{
    ast::{
        binary_op::{BinaryOp, BinaryOperator, BinaryOperatorKind},
        display::AstDisplay,
        expr::{AlternateCondition, Expr, FnCall, IfCondition},
        Block, LetDeclare,
    },
    error::ErrorSpan,
    ir::{DeclarationId, Module, Rir},
};

type ResultValue = Result<RainValue, ErrorSpan<RunnerError>>;

struct Cx<'a> {
    module: &'a Module<'a>,
    call_depth: usize,
    args: HashMap<&'a str, RainValue>,
}

impl<'a> Cx<'a> {
    fn new(module: &'a Module<'a>) -> Self {
        Self {
            module,
            call_depth: 0,
            args: HashMap::new(),
        }
    }
}

pub struct Runner<'a> {
    rir: &'a Rir<'a>,
    cache: cache::Cache,
}

impl<'a> Runner<'a> {
    pub fn new(rir: &'a Rir<'a>) -> Self {
        Self {
            rir,
            cache: cache::Cache::default(),
        }
    }

    pub fn evaluate_and_call(&mut self, id: DeclarationId) -> ResultValue {
        let v = self.evaluate(id)?;
        if v.rain_type_id() == TypeId::of::<RainFunction>() {
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
                self.evaluate_expr(&Cx::new(m), expr)
            }
            crate::ast::Declaration::FnDeclare(_) => Ok(RainValue::new(RainFunction { id })),
        }
    }

    fn call_function(
        &mut self,
        call_depth: usize,
        RainFunction { id }: &RainFunction,
        arg_values: Vec<RainValue>,
    ) -> ResultValue {
        let m = self.rir.get_module(id.module_id());
        let d = m.get_declaration(id.local_id());
        match d {
            crate::ast::Declaration::FnDeclare(fn_declare) => {
                let args = fn_declare
                    .args
                    .iter()
                    .zip(arg_values)
                    .map(|(a, v)| (a.name.span.contents(m.src), v))
                    .collect();
                let cx = Cx {
                    module: m,
                    call_depth,
                    args,
                };
                self.evaluate_block(&cx, &fn_declare.block)
            }
            _ => unreachable!(),
        }
    }

    fn evaluate_block(&mut self, cx: &Cx, block: &Block) -> ResultValue {
        let crate::ast::Statement::Expr(expr) = block.statements.last().unwrap();
        self.evaluate_expr(cx, expr)
    }

    fn resolve_ident(
        &mut self,
        cx: &Cx,
        ident: &str,
    ) -> Result<Option<RainValue>, ErrorSpan<RunnerError>> {
        if let Some(v) = cx.args.get(ident) {
            return Ok(Some(v.clone()));
        }
        let Some(declaration_id) = self.rir.resolve_global_declaration(cx.module.id, ident) else {
            return Ok(None);
        };
        Ok(Some(self.evaluate(declaration_id)?))
    }

    fn evaluate_expr(&mut self, cx: &Cx, expr: &Expr) -> ResultValue {
        match expr {
            Expr::Ident(tls) => {
                let ident_name = tls.span.contents(cx.module.src);
                self.resolve_ident(cx, ident_name)?
                    .ok_or(tls.span.with_error(RunnerError::UnknownIdent))
            }
            Expr::StringLiteral(tls) => {
                let mut string_value = tls.span.contents(cx.module.src);
                let mut prefix = None;
                match string_value.chars().next() {
                    Some(p @ 'a'..='z') => {
                        string_value = &string_value[1..];
                        prefix = Some(p);
                    }
                    Some('"') => (),
                    Some(_) => panic!("unrecognised string prefix"),
                    None => unreachable!("empty string literal"),
                }
                string_value = string_value
                    .strip_prefix('"')
                    .expect("strip prefix double quote")
                    .strip_suffix('\"')
                    .expect("strip suffix double quote");
                match prefix {
                    Some('f') => todo!("format string"),
                    Some(p) => panic!("unrecognised string prefix: {p}"),
                    None => (),
                }
                Ok(RainValue::new(string_value.to_owned()))
            }
            Expr::IntegerLiteral(tls) => Ok(RainValue::new(
                tls.span
                    .contents(cx.module.src)
                    .parse::<RainInteger>()
                    .unwrap(),
            )),
            Expr::TrueLiteral(_) => Ok(RainValue::new(true)),
            Expr::FalseLiteral(_) => Ok(RainValue::new(false)),
            Expr::BinaryOp(b) => self.evaluate_binary_op(cx, b),
            Expr::FnCall(fn_call) => self.evaluate_fn_call(cx, fn_call),
            Expr::If(if_condition) => self.evaluate_if_condition(cx, if_condition),
        }
    }

    fn evaluate_fn_call(&mut self, cx: &Cx, fn_call: &FnCall) -> ResultValue {
        let v = self.evaluate_expr(cx, &fn_call.callee)?;
        if v.rain_type_id() != TypeId::of::<RainFunction>() {
            return Err(fn_call
                .callee
                .span()
                .with_error(RunnerError::GenericTypeError));
        }
        let Some(f) = v.downcast::<RainFunction>() else {
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
        let key = self.cache.function_call_key(&f, &arg_values);

        if let Some(v) = self.cache.get(&key) {
            return Ok(v.clone());
        }
        let v = self.call_function(cx.call_depth + 1, &f, arg_values)?;
        self.cache.put(key, v.clone());
        Ok(v)
    }

    fn evaluate_binary_op(
        &mut self,
        cx: &Cx,
        BinaryOp {
            left,
            op: BinaryOperator { kind, .. },
            right,
        }: &BinaryOp,
    ) -> ResultValue {
        let left_value = self.evaluate_expr(cx, left)?;
        let right_value = self.evaluate_expr(cx, right)?;
        match kind {
            BinaryOperatorKind::Addition => Ok(RainValue::new(RainInteger(
                left_value.downcast::<RainInteger>().unwrap().0
                    + right_value.downcast::<RainInteger>().unwrap().0,
            ))),
            BinaryOperatorKind::Subtraction => Ok(RainValue::new(RainInteger(
                left_value.downcast::<RainInteger>().unwrap().0
                    - right_value.downcast::<RainInteger>().unwrap().0,
            ))),
            BinaryOperatorKind::Multiplication => Ok(RainValue::new(RainInteger(
                left_value.downcast::<RainInteger>().unwrap().0
                    * right_value.downcast::<RainInteger>().unwrap().0,
            ))),
            BinaryOperatorKind::Division => Ok(RainValue::new(RainInteger(
                left_value.downcast::<RainInteger>().unwrap().0
                    / right_value.downcast::<RainInteger>().unwrap().0,
            ))),
            BinaryOperatorKind::Dot => todo!("evaluate dot expr"),
            BinaryOperatorKind::LogicalAnd => Ok(RainValue::new(
                *left_value.downcast::<bool>().unwrap() && *right_value.downcast::<bool>().unwrap(),
            )),
            BinaryOperatorKind::LogicalOr => Ok(RainValue::new(
                *left_value.downcast::<bool>().unwrap() || *right_value.downcast::<bool>().unwrap(),
            )),
            BinaryOperatorKind::Equals => Ok(RainValue::new(
                left_value.downcast::<RainInteger>().unwrap().0
                    == right_value.downcast::<RainInteger>().unwrap().0,
            )),
            BinaryOperatorKind::NotEquals => todo!("evaluate not equality"),
        }
    }

    fn evaluate_if_condition(
        &mut self,
        cx: &Cx,
        IfCondition {
            condition,
            then,
            alternate,
        }: &IfCondition,
    ) -> ResultValue {
        let condition_value = self.evaluate_expr(cx, condition)?;
        let Some(condition_bool): Option<Arc<bool>> = condition_value.downcast() else {
            return Err(condition.span().with_error(RunnerError::GenericTypeError));
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
