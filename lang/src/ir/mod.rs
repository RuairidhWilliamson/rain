#[cfg(test)]
mod test;

use std::path::PathBuf;

use crate::{
    ast::{
        binary_op::BinaryOp,
        expr::{AlternateCondition, Expr, FnCall, FnCallArgs, IfCondition},
        Block, Declaration, FnDeclare,
    },
    error::ErrorLocalSpan,
};

#[derive(Debug, Default)]
pub struct Rir {
    modules: Vec<Module>,
}

impl Rir {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_module(
        &mut self,
        path: Option<PathBuf>,
        src: String,
        ast: crate::ast::Script,
    ) -> ModuleId {
        let declarations = ast.declarations;
        let id = ModuleId(self.modules.len());
        self.modules.push(Module {
            id,
            path,
            src,
            declarations,
        });
        id
    }

    pub fn get_module(&self, module_id: ModuleId) -> &Module {
        let Some(m) = self.modules.get(module_id.0) else {
            unreachable!("id is always valid")
        };
        m
    }

    pub fn resolve_global_declaration(
        &self,
        module_id: ModuleId,
        name: &str,
    ) -> Option<DeclarationId> {
        self.get_module(module_id)
            .find_declaration_by_name(name)
            .map(|id| DeclarationId(module_id, id))
    }

    pub fn declaration_deps(
        &self,
        id: DeclarationId,
    ) -> Result<Vec<DeclarationId>, ErrorLocalSpan<RainError>> {
        let module = self.get_module(id.module_id());
        let declaration = module.get_declaration(id.local_id());
        Ok(match declaration {
            Declaration::LetDeclare(declare) => module.expr_deps(&declare.expr)?,
            Declaration::FnDeclare(FnDeclare { block, .. }) => module.block_deps(block)?,
        }
        .into_iter()
        .map(|i| DeclarationId(id.0, i))
        .collect())
    }
}

#[derive(Debug)]
pub struct Module {
    pub id: ModuleId,
    #[allow(dead_code)]
    path: Option<PathBuf>,
    pub src: String,
    declarations: Vec<Declaration>,
}

impl Module {
    pub fn get_declaration(&self, id: LocalDeclarationId) -> &Declaration {
        let Some(d) = self.declarations.get(id.0) else {
            unreachable!("id is always valid");
        };
        d
    }

    fn expr_deps(&self, expr: &Expr) -> Result<Vec<LocalDeclarationId>, ErrorLocalSpan<RainError>> {
        let mut v = Vec::new();
        match expr {
            Expr::Ident(tls) => {
                v.push(
                    self.find_declaration_by_name(tls.span.contents(&self.src))
                        .ok_or_else(|| tls.span.with_error(RainError::UnresolvedIdentifier))?,
                );
            }
            Expr::StringLiteral(_)
            | Expr::IntegerLiteral(_)
            | Expr::TrueLiteral(_)
            | Expr::FalseLiteral(_)
            | Expr::Internal(_) => {}
            Expr::BinaryOp(BinaryOp { left, op: _, right }) => {
                v.extend(self.expr_deps(left)?);
                v.extend(self.expr_deps(right)?);
            }
            Expr::FnCall(FnCall {
                callee,
                args: FnCallArgs { args, .. },
            }) => {
                v.extend(self.expr_deps(callee)?);
                for a in args {
                    v.extend(self.expr_deps(a)?);
                }
            }
            Expr::If(if_condition) => v.extend(self.if_condition_deps(if_condition)?),
        }
        Ok(v)
    }

    fn if_condition_deps(
        &self,
        if_condition: &IfCondition,
    ) -> Result<Vec<LocalDeclarationId>, ErrorLocalSpan<RainError>> {
        let mut v = Vec::new();
        let IfCondition {
            condition,
            then,
            alternate,
        } = if_condition;
        v.extend(self.expr_deps(condition)?);
        v.extend(self.block_deps(then)?);
        match alternate {
            Some(AlternateCondition::IfElse(if_condition)) => {
                v.extend(self.if_condition_deps(if_condition)?);
            }
            Some(AlternateCondition::Else(block)) => {
                v.extend(self.block_deps(block)?);
            }
            None => (),
        }

        Ok(v)
    }

    fn block_deps(
        &self,
        block: &Block,
    ) -> Result<Vec<LocalDeclarationId>, ErrorLocalSpan<RainError>> {
        let mut v = Vec::new();
        for s in &block.statements {
            match s {
                crate::ast::Statement::Expr(expr) => {
                    v.extend(self.expr_deps(expr)?);
                }
                crate::ast::Statement::Assignment(assign) => {
                    v.extend(self.expr_deps(&assign.expr)?);
                }
            }
        }
        Ok(v)
    }

    fn find_declaration_by_name(&self, name: &str) -> Option<LocalDeclarationId> {
        self.declarations
            .iter()
            .enumerate()
            .find(|(_, d)| d.name().span.contents(&self.src) == name)
            .map(|(id, _)| LocalDeclarationId(id))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct ModuleId(usize);

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct LocalDeclarationId(usize);

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct DeclarationId(ModuleId, LocalDeclarationId);

impl DeclarationId {
    pub fn module_id(&self) -> ModuleId {
        self.0
    }

    pub fn local_id(&self) -> LocalDeclarationId {
        self.1
    }
}

#[derive(Debug)]
pub enum RainError {
    UnresolvedIdentifier,
}

impl std::fmt::Display for RainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnresolvedIdentifier => f.write_str("unresolved identifier"),
        }
    }
}

impl std::error::Error for RainError {}
