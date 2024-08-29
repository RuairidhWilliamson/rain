pub mod value;

use value::RainValue;

use crate::{
    ast::{
        expr::{BinaryOp, BinaryOperator, BinaryOperatorKind, Expr},
        FnDeclare, LetDeclare,
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
        let crate::ast::Statement::Expr(expr) = fn_declare.block.statements.last().unwrap();
        self.evaluate_expr(module, expr)
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
            Expr::StringLiteral(tls) => RainValue::new(tls.span.contents(module.src).to_owned()),
            Expr::IntegerLiteral(tls) => {
                RainValue::new(tls.span.contents(module.src).parse::<isize>().unwrap())
            }
            Expr::TrueLiteral(_) => RainValue::new(true),
            Expr::FalseLiteral(_) => RainValue::new(false),
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
}
