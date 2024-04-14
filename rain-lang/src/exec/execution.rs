use crate::{
    ast::{
        block::Block, declare::Declare, dot::Dot, expr::Expr, function_call::FnCall,
        function_def::FnDef, ident::Ident, if_condition::IfCondition, list_literal::ListLiteral,
        return_stmt::Return, script::Script, statement::Statement, statement_list::StatementList,
        Ast,
    },
    error::RainError,
};

use super::{
    executor::Executor,
    types::{function::Function, RainType, RainValue},
    ExecCF, ExecError,
};

pub trait Execution {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, ExecCF>;
}

impl Execution for Script {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, ExecCF> {
        self.statements.execute(executor)
    }
}

impl Execution for Block {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, ExecCF> {
        self.stmts.execute(executor)
    }
}

impl Execution for StatementList {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, ExecCF> {
        let mut out = RainValue::Void;
        for stmt in &self.statements {
            out = stmt.execute(executor)?;
        }
        Ok(out)
    }
}

impl Execution for Statement {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, ExecCF> {
        match self {
            Self::Expr(expr) => expr.execute(executor),
            Self::LetDeclare(declare) => declare.execute(executor),
            Self::LazyDeclare(declare) => declare.execute(executor),
            Self::FnDef(fndef) => fndef.execute(executor),
            Self::Return(ret) => ret.execute(executor),
        }
    }
}

impl Execution for Expr {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, ExecCF> {
        match self {
            Self::Ident(ident) => ident.execute(executor),
            Self::Dot(dot) => dot.execute(executor),
            Self::FnCall(fn_call) => fn_call.execute(executor),
            Self::BoolLiteral(inner) => Ok(RainValue::Bool(inner.value)),
            Self::StringLiteral(inner) => Ok(RainValue::String(inner.value.as_str().into())),
            Self::ListLiteral(array) => array.execute(executor),
            Self::IfCondition(inner) => inner.execute(executor),
            Self::Match(_) => todo!(),
        }
    }
}

impl Execution for FnDef {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, ExecCF> {
        let source = executor.script_executor.source.clone();
        executor.global_record().insert(
            self.name.name.to_owned(),
            RainValue::Function(Function::new(source, self.clone())),
        );
        Ok(RainValue::Void)
    }
}

impl Execution for Ident {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, ExecCF> {
        executor
            .resolve(&self.name)
            .ok_or(ExecCF::RainError(RainError::new(
                ExecError::UnknownItem(self.name.to_string()),
                self.span,
            )))
    }
}

impl Execution for Dot {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, ExecCF> {
        let Some(left) = &self.left else {
            return Err(RainError::new(
                ExecError::Roadmap("context aware dot expressions are roadmapped"),
                self.dot_token,
            )
            .into());
        };
        let left_value = left.execute(executor)?;
        let RainValue::Record(record) = left_value else {
            return Err(RainError::new(
                ExecError::UnexpectedType {
                    expected: &[RainType::Record],
                    actual: left_value.as_type(),
                },
                left.span(),
            )
            .into());
        };
        record
            .get(&self.right.name)
            .ok_or(ExecCF::RainError(RainError::new(
                ExecError::UnknownItem(self.right.name.to_string()),
                self.right.span(),
            )))
    }
}

impl Execution for FnCall {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, ExecCF> {
        let fn_value = self.expr.execute(executor)?;
        let RainValue::Function(func) = fn_value else {
            return Err(RainError::new(
                ExecError::UnexpectedType {
                    expected: &[RainType::Function],
                    actual: fn_value.as_type(),
                },
                self.expr.span(),
            )
            .into());
        };

        let args = self
            .args
            .iter()
            .map(|a| a.value.execute(executor).map(|v| (&a.name, v)))
            .collect::<Result<Vec<(&Option<Ident>, RainValue)>, ExecCF>>()?;
        func.call(executor, &args, Some(self))
    }
}

impl Execution for Declare {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, ExecCF> {
        let value = self.value.execute(executor)?;
        executor
            .global_record()
            .insert(self.name.name.to_owned(), value);
        Ok(RainValue::Void)
    }
}

impl Execution for Return {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, ExecCF> {
        let value = self.expr.execute(executor)?;
        Err(ExecCF::Return(value))
    }
}

impl Execution for IfCondition {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, ExecCF> {
        let condition_value = self.condition.execute(executor)?;
        let RainValue::Bool(v) = condition_value else {
            return Err(RainError::new(
                ExecError::UnexpectedType {
                    expected: &[RainType::Bool],
                    actual: condition_value.as_type(),
                },
                self.condition.span(),
            )
            .into());
        };
        if v {
            self.then_block.execute(executor)
        } else if let Some(else_condition) = &self.else_condition {
            else_condition.block.execute(executor)
        } else {
            Ok(RainValue::Void)
        }
    }
}

impl Execution for ListLiteral {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, ExecCF> {
        let elements = self
            .elements
            .iter()
            .map(|e| e.execute(executor))
            .collect::<Result<Vec<RainValue>, ExecCF>>()?;
        Ok(RainValue::List(elements.into()))
    }
}
