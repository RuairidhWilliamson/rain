use std::{borrow::Cow, collections::HashSet};

use crate::{
    ir::{ModuleId, Rir},
    span::ErrorSpan,
};

#[derive(Debug, thiserror::Error)]
pub enum CheckError {
    #[error("makeshift: {0}")]
    Makeshift(Cow<'static, str>),
}

pub struct Checker<'a> {
    pub ir: &'a mut Rir,
}

impl<'a> Checker<'a> {
    pub fn new(rir: &'a mut Rir) -> Self {
        Self { ir: rir }
    }

    pub fn check_module(&mut self, mid: ModuleId) -> Result<(), ErrorSpan<CheckError>> {
        let module = self.ir.get_module(mid);
        let mut declaration_names = HashSet::new();
        for d in module.declarations() {
            for name_span in d.assignment.name_spans() {
                let name = name_span.contents(&module.src);
                if !declaration_names.insert(name) {
                    return Err(name_span
                        .with_error(CheckError::Makeshift(
                            format!("multiple declarations with the same name {name}").into(),
                        ))
                        .upgrade(mid));
                }
            }
        }
        Ok(())
    }
}
