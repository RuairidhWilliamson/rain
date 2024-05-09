use ordered_hash_map::OrderedHashMap;

use crate::{
    ast::{
        declaration::{Declaration, InnerDeclaration},
        script::Script,
        Ast,
    },
    error::RainError,
    exec::{
        execution::Execution,
        types::{function::Function, RainValue},
        ExecCF, ExecError,
    },
    executor::{base::BaseExecutor, Executor},
    source::Source,
};

#[derive(Debug)]
pub struct ScriptExecutor {
    declarations: OrderedHashMap<String, Declaration>,
    source: Source,
}

impl ScriptExecutor {
    pub fn new(script: Script, source: Source) -> Result<Self, RainError> {
        let mut declarations = OrderedHashMap::<String, Declaration>::new();
        for d in script.declarations {
            let name = d.inner.name();
            if let Some(old_d) = declarations.get(name) {
                return Err(RainError::new(
                    ExecError::DuplicateDeclare(old_d.span()),
                    d.span(),
                ));
            }
            declarations.insert(name.to_owned(), d);
        }
        Ok(Self {
            declarations,
            source,
        })
    }

    pub fn source(&self) -> &Source {
        &self.source
    }

    pub fn get(&self, name: &str) -> Option<&Declaration> {
        self.declarations.get(name)
    }

    pub fn resolve(
        &self,
        name: &str,
        base_executor: &mut BaseExecutor,
    ) -> Option<Result<RainValue, ExecCF>> {
        let d = self.get(name)?;
        let mut executor = Executor::new(base_executor, self);
        Some(match d {
            Declaration {
                inner: InnerDeclaration::Let(inner),
                ..
            } => inner.value.execute(&mut executor),
            Declaration {
                inner: InnerDeclaration::Lazy(inner),
                ..
            } => inner.value.execute(&mut executor),
            Declaration {
                inner: InnerDeclaration::Function(function),
                ..
            } => Ok(RainValue::Function(Function::new(
                self.source.clone(),
                function.clone(),
            ))),
        })
    }
}

impl<'a> IntoIterator for &'a ScriptExecutor {
    type Item = (&'a String, &'a Declaration);
    type IntoIter = ordered_hash_map::ordered_map::Iter<'a, String, Declaration>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIterator::into_iter(&self.declarations)
    }
}
