use std::{
    any::{Any, TypeId},
    fmt::Debug,
};

use crate::{
    ast::{
        expr::{BinaryOp, BinaryOperator, BinaryOperatorKind, Expr},
        FnDeclare, LetDeclare,
    },
    ir::{DeclarationId, Rir},
};

use super::Module;

pub struct Runner<'a> {
    rir: &'a Rir<'a>,
}

impl<'a> Runner<'a> {
    pub fn new(rir: &'a Rir<'a>) -> Self {
        Self { rir }
    }

    pub fn evaluate(&mut self, id: DeclarationId) -> RainValue {
        let m = self.rir.get_module(id.module_id());
        let d = m.get_declaration(id.local_id());
        match d {
            crate::ast::Declaration::LetDeclare(LetDeclare { expr, .. }) => {
                self.evaluate_expr(m, expr)
            }
            crate::ast::Declaration::FnDeclare(fn_declare) => self.evaluate_fn(m, fn_declare),
        }
    }

    fn evaluate_fn(&mut self, module: &Module, fn_declare: &FnDeclare) -> RainValue {
        todo!()
    }

    fn evaluate_expr(&mut self, module: &Module, expr: &Expr) -> RainValue {
        match expr {
            Expr::Ident(tls) => {
                let Some(declaration_id) = self
                    .rir
                    .resolve_global_declaration(module.id, tls.span.contents(module.src))
                else {
                    todo!();
                };
                self.evaluate(declaration_id)
            }
            Expr::StringLiteral(_) => todo!("evaluate string literal"),
            Expr::IntegerLiteral(tls) => RainValue {
                value: Box::new(tls.span.contents(module.src).parse::<isize>().unwrap()),
            },
            Expr::TrueLiteral(_) => todo!("evaluate true literal"),
            Expr::FalseLiteral(_) => todo!("evaluate false literal"),
            Expr::BinaryOp(b) => self.evaluate_binary_op(module, b),
            Expr::FnCall(_) => todo!("evaluate fn call"),
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
            BinaryOperatorKind::LogicalAnd => todo!("evaluate logical and"),
            BinaryOperatorKind::LogicalOr => todo!("evaluate logical or"),
            BinaryOperatorKind::Equals => todo!("evaluate equality"),
            BinaryOperatorKind::NotEquals => todo!("evaluate not equality"),
        }
    }
}

#[derive(Debug)]
pub struct RainValue {
    value: Box<dyn RainValueInner>,
}

impl RainValue {
    fn new<T: RainValueInner>(value: T) -> Self {
        Self {
            value: Box::new(value),
        }
    }

    pub fn downcast<T: RainValueInner>(self) -> Option<Box<T>> {
        if (*self.value).type_id() == TypeId::of::<T>() {
            let ptr = Box::into_raw(self.value);
            // Safety:
            // We have checked this is of the right type already
            Some(unsafe { Box::from_raw(ptr.cast()) })
        } else {
            None
        }
    }
}

pub trait RainValueInner: Any + Debug + Send + Sync {}

impl RainValueInner for isize {}
