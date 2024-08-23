use std::any::Any;

use crate::{
    ast::{
        expr::{BinaryOp, BinaryOperator, BinaryOperatorKind, Expr},
        LetDeclare,
    },
    ir::{DeclarationId, Rir},
};

pub struct Runner<'a> {
    pub rir: Rir<'a>,
    evaluator: Evaluator,
}

impl<'a> Runner<'a> {
    pub fn new(rir: Rir<'a>) -> Self {
        Self {
            rir,
            evaluator: Evaluator {},
        }
    }

    pub fn evaluate(&mut self, id: DeclarationId) {
        let m = self.rir.get_module(id.module_id());
        let d = m.get_declaration(id.local_id());
        let expr = match d {
            crate::ast::Declaration::LetDeclare(LetDeclare { expr, .. }) => expr,
            crate::ast::Declaration::FnDeclare(_) => todo!("run main fn"),
        };
        let v = self.evaluator.evaluate_expr(m.src, expr);
        let v_debug = v.value.downcast_ref::<isize>().unwrap();
        println!("{v_debug:?}");
    }
}

struct Evaluator {}

impl Evaluator {
    fn evaluate_expr(&mut self, src: &str, expr: &Expr) -> RainValue {
        match expr {
            Expr::Ident(_) => todo!("evaluate ident"),
            Expr::StringLiteral(_) => todo!("evaluate string literal"),
            Expr::IntegerLiteral(tls) => RainValue {
                value: Box::new(tls.span.contents(src).parse::<isize>().unwrap()),
            },
            Expr::TrueLiteral(_) => todo!("evaluate true literal"),
            Expr::FalseLiteral(_) => todo!("evaluate false literal"),
            Expr::BinaryOp(b) => self.evaluate_binary_op(src, b),
            Expr::FnCall(_) => todo!("evaluate fn call"),
        }
    }

    fn evaluate_binary_op(
        &mut self,
        src: &str,
        BinaryOp {
            left,
            op: BinaryOperator { kind, .. },
            right,
        }: &BinaryOp,
    ) -> RainValue {
        let left_value = self.evaluate_expr(src, left);
        let right_value = self.evaluate_expr(src, right);
        match kind {
            BinaryOperatorKind::Addition => RainValue {
                value: Box::new(
                    *left_value.value.downcast::<isize>().unwrap()
                        + *right_value.value.downcast::<isize>().unwrap(),
                ),
            },
            BinaryOperatorKind::Subtraction => RainValue {
                value: Box::new(
                    *left_value.value.downcast::<isize>().unwrap()
                        - *right_value.value.downcast::<isize>().unwrap(),
                ),
            },
            BinaryOperatorKind::Multiplication => RainValue {
                value: Box::new(
                    *left_value.value.downcast::<isize>().unwrap()
                        * *right_value.value.downcast::<isize>().unwrap(),
                ),
            },
            BinaryOperatorKind::Division => RainValue {
                value: Box::new(
                    *left_value.value.downcast::<isize>().unwrap()
                        / *right_value.value.downcast::<isize>().unwrap(),
                ),
            },
            BinaryOperatorKind::Dot => todo!("evaluate dot expr"),
            BinaryOperatorKind::LogicalAnd => todo!("evaluate logical and"),
            BinaryOperatorKind::LogicalOr => todo!("evaluate logical or"),
            BinaryOperatorKind::Equals => todo!("evaluate equality"),
            BinaryOperatorKind::NotEquals => todo!("evaluate not equality"),
        }
    }
}

#[derive(Debug)]
struct RainValue {
    value: Box<dyn Any>,
}
