pub mod value;

use std::any::TypeId;

use value::{RainFunction, RainValue};

use crate::{
    ast::{
        binary_op::{BinaryOp, BinaryOperator, BinaryOperatorKind},
        expr::{AlternateCondition, Expr, FnCall, IfCondition},
        Block, LetDeclare,
    },
    ir::{DeclarationId, Module, Rir},
};

pub struct Runner<'a> {
    rir: &'a Rir<'a>,
}

impl<'a> Runner<'a> {
    pub fn new(rir: &'a Rir<'a>) -> Self {
        Self { rir }
    }

    pub fn evaluate_and_call(&mut self, id: DeclarationId) -> RainValue {
        let v = self.evaluate(id);
        if v.rain_type_id() == TypeId::of::<RainFunction>() {
            let Some(f) = v.downcast::<RainFunction>() else {
                unreachable!();
            };
            self.evaluate_fn(&f)
        } else {
            v
        }
    }

    pub fn evaluate(&mut self, id: DeclarationId) -> RainValue {
        let m = self.rir.get_module(id.module_id());
        let d = m.get_declaration(id.local_id());
        match d {
            crate::ast::Declaration::LetDeclare(LetDeclare { expr, .. }) => {
                self.evaluate_expr(m, expr)
            }
            crate::ast::Declaration::FnDeclare(_) => RainValue::new(RainFunction { id }),
        }
    }

    fn evaluate_fn(&mut self, RainFunction { id }: &RainFunction) -> RainValue {
        let m = self.rir.get_module(id.module_id());
        let d = m.get_declaration(id.local_id());
        match d {
            crate::ast::Declaration::FnDeclare(fn_declare) => {
                self.evaluate_block(m, &fn_declare.block)
            }
            _ => unreachable!(),
        }
    }

    fn evaluate_block(&mut self, module: &Module, block: &Block) -> RainValue {
        let crate::ast::Statement::Expr(expr) = block.statements.last().unwrap();
        self.evaluate_expr(module, expr)
    }

    fn evaluate_expr(&mut self, module: &Module, expr: &Expr) -> RainValue {
        match expr {
            Expr::Ident(tls) => {
                let ident_name = tls.span.contents(module.src);
                let Some(declaration_id) =
                    self.rir.resolve_global_declaration(module.id, ident_name)
                else {
                    panic!("unknown ident: {ident_name}");
                };
                self.evaluate(declaration_id)
            }
            Expr::StringLiteral(tls) => {
                let mut string_value = tls.span.contents(module.src);
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
                RainValue::new(string_value.to_owned())
            }
            Expr::IntegerLiteral(tls) => {
                RainValue::new(tls.span.contents(module.src).parse::<isize>().unwrap())
            }
            Expr::TrueLiteral(_) => RainValue::new(true),
            Expr::FalseLiteral(_) => RainValue::new(false),
            Expr::BinaryOp(b) => self.evaluate_binary_op(module, b),
            Expr::FnCall(fn_call) => self.evaluate_fn_call(module, fn_call),
            Expr::If(if_condition) => self.evaluate_if_condition(module, if_condition),
        }
    }

    fn evaluate_fn_call(&mut self, module: &Module, FnCall { callee, args }: &FnCall) -> RainValue {
        let _ = args;
        let v = self.evaluate_expr(module, callee);
        if v.rain_type_id() == TypeId::of::<RainFunction>() {
            let Some(f) = v.downcast::<RainFunction>() else {
                unreachable!();
            };
            self.evaluate_fn(&f)
        } else {
            panic!("can't call value: {v:?}")
        }
    }

    fn evaluate_binary_op(
        &mut self,
        module: &Module,
        BinaryOp {
            left,
            op: BinaryOperator { kind, .. },
            right,
        }: &BinaryOp,
    ) -> RainValue {
        let left_value = self.evaluate_expr(module, left);
        let right_value = self.evaluate_expr(module, right);
        match kind {
            BinaryOperatorKind::Addition => RainValue::new(
                *left_value.downcast::<isize>().unwrap()
                    + *right_value.downcast::<isize>().unwrap(),
            ),
            BinaryOperatorKind::Subtraction => RainValue::new(
                *left_value.downcast::<isize>().unwrap()
                    - *right_value.downcast::<isize>().unwrap(),
            ),
            BinaryOperatorKind::Multiplication => RainValue::new(
                *left_value.downcast::<isize>().unwrap()
                    * *right_value.downcast::<isize>().unwrap(),
            ),
            BinaryOperatorKind::Division => RainValue::new(
                *left_value.downcast::<isize>().unwrap()
                    / *right_value.downcast::<isize>().unwrap(),
            ),
            BinaryOperatorKind::Dot => todo!("evaluate dot expr"),
            BinaryOperatorKind::LogicalAnd => RainValue::new(
                *left_value.downcast::<bool>().unwrap() && *right_value.downcast::<bool>().unwrap(),
            ),
            BinaryOperatorKind::LogicalOr => RainValue::new(
                *left_value.downcast::<bool>().unwrap() || *right_value.downcast::<bool>().unwrap(),
            ),
            BinaryOperatorKind::Equals => todo!("evaluate equality"),
            BinaryOperatorKind::NotEquals => todo!("evaluate not equality"),
        }
    }

    fn evaluate_if_condition(
        &mut self,
        module: &Module,
        IfCondition {
            condition,
            then,
            alternate,
        }: &IfCondition,
    ) -> RainValue {
        let condition_value = self.evaluate_expr(module, condition);
        let Some(condition_bool): Option<Box<bool>> = condition_value.downcast() else {
            panic!("condition is not bool");
        };
        if *condition_bool {
            self.evaluate_block(module, then)
        } else {
            match alternate {
                Some(AlternateCondition::IfElse(if_condition)) => {
                    self.evaluate_if_condition(module, if_condition)
                }
                Some(AlternateCondition::Else(block)) => self.evaluate_block(module, block),
                None => RainValue::new(()),
            }
        }
    }
}
