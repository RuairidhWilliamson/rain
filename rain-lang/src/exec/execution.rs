use crate::{
    ast::{
        block::Block, declare::Declare, expr::Expr, function_call::FnCall, function_def::FnDef,
        if_condition::IfCondition, item::Item, return_stmt::Return, script::Script,
        statement::Statement, statement_list::StatementList, Ast,
    },
    error::RainError,
};

use super::{
    executor::Executor,
    types::{self, function::Function, RainType, RainValue},
    ExecCF, ExecError,
};

pub trait Execution {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, ExecCF>;
}

impl Execution for Script<'static> {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, ExecCF> {
        self.statements.execute(executor)
    }
}

impl Execution for Block<'static> {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, ExecCF> {
        self.stmts.execute(executor)
    }
}

impl Execution for StatementList<'static> {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, ExecCF> {
        let mut out = RainValue::Void;
        for stmt in &self.statements {
            out = stmt.execute(executor)?;
        }
        Ok(out)
    }
}

impl Execution for Statement<'static> {
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

impl Execution for Expr<'static> {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, ExecCF> {
        match self {
            Self::Item(item) => item.execute(executor),
            Self::FnCall(fn_call) => fn_call.execute(executor),
            Self::BoolLiteral(inner) => Ok(RainValue::Bool(inner.value)),
            Self::StringLiteral(inner) => Ok(RainValue::String((inner.value).into())),
            Self::IfCondition(inner) => inner.execute(executor),
            Self::Match(_) => todo!(),
        }
    }
}

impl Execution for FnDef<'static> {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, ExecCF> {
        executor.global_executor().global_record.insert(
            self.name.name.to_owned(),
            RainValue::Function(Function::new(self.clone())),
        );
        Ok(RainValue::Void)
    }
}

impl Execution for FnCall<'static> {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, ExecCF> {
        let fn_value = self.item.execute(executor)?;
        let RainValue::Function(func) = fn_value else {
            return Err(RainError::new(
                ExecError::UnexpectedType {
                    expected: &[types::RainType::Function],
                    actual: fn_value.as_type(),
                },
                self.item.span,
            )
            .into());
        };

        let args = self
            .args
            .iter()
            .map(|a| a.execute(executor))
            .collect::<Result<Vec<RainValue>, ExecCF>>()?;
        func.call(executor, &args, self)
    }
}

impl Execution for Declare<'static> {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, ExecCF> {
        let value = self.value.execute(executor)?;
        executor
            .global_executor()
            .global_record
            .insert(self.name.name.to_owned(), value);
        Ok(RainValue::Void)
    }
}

impl Execution for Item<'static> {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, ExecCF> {
        let (top_level, rest) = self.idents.split_first().unwrap();
        let mut record = executor
            .local_record
            .get(top_level.name)
            .or_else(|| executor.global_executor().global_record.get(top_level.name))
            .ok_or(RainError::new(
                ExecError::UnknownVariable(top_level.name.to_owned()),
                top_level.span,
            ))?;
        for ident in rest {
            record = record
                .as_record()
                .map_err(|typ| {
                    RainError::new(
                        ExecError::UnexpectedType {
                            expected: &[types::RainType::Record],
                            actual: typ,
                        },
                        ident.span,
                    )
                })?
                .get(ident.name)
                .ok_or_else(|| {
                    RainError::new(ExecError::UnknownItem(String::from(ident.name)), ident.span)
                })?;
        }
        Ok(record.to_owned())
    }
}

impl Execution for Return<'static> {
    fn execute(&self, executor: &mut Executor) -> Result<RainValue, ExecCF> {
        let value = self.expr.execute(executor)?;
        Err(ExecCF::Return(value))
    }
}

impl Execution for IfCondition<'static> {
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
