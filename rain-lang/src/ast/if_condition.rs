use super::expr::Expr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfCondition<'a> {
    pub condition: Box<Expr<'a>>,
}

impl<'a> IfCondition<'a> {
    pub fn reset_spans(&mut self) {
        self.condition.reset_spans();
    }
}
