use std::{collections::HashMap, sync::Arc};

use indexmap::IndexMap;

use crate::{
    afs::area::FileArea,
    ast::NodeId,
    ir::{DeclarationId, IrModule, ModuleId},
    local_span::LocalSpan,
    runner::{
        error::{ErrorTrace, RunnerError, Throwing},
        value::Value,
    },
};

use super::dep::Dep;

pub struct Cx<'a> {
    pub module: &'a Arc<IrModule>,
    pub call_depth: usize,
    pub locals: HashMap<&'a str, Value>,
    pub captures: Vec<Arc<IndexMap<String, Value>>>,
    pub args: HashMap<&'a str, Value>,
    pub deps: Vec<Dep>,
    pub previous_line: Option<Value>,
    pub stacktrace: Vec<StacktraceEntry>,
}

#[derive(Debug, Clone)]
pub struct StacktraceEntry {
    pub m: ModuleId,
    pub n: NodeId,
    pub d: DeclarationId,
}

impl<'a> Cx<'a> {
    pub fn new(
        module: &'a Arc<IrModule>,
        call_depth: usize,
        args: HashMap<&'a str, Value>,
        stacktrace: Vec<StacktraceEntry>,
    ) -> Self {
        Self {
            module,
            call_depth,
            args,
            captures: Vec::new(),
            locals: HashMap::new(),
            deps: Vec::new(),
            previous_line: None,
            stacktrace,
        }
    }

    pub fn err(&self, s: impl Into<LocalSpan>, err: RunnerError) -> ErrorTrace<Throwing> {
        s.into()
            .with_module(self.module.id)
            .with_error(err.into())
            .with_trace(self.stacktrace.clone())
    }

    pub fn nid_err(&self, nid: impl Into<NodeId>, err: RunnerError) -> ErrorTrace<Throwing> {
        self.err(self.module.span(nid.into()), err)
    }

    pub fn add_dep_file_area(&mut self, area: &FileArea) {
        match area {
            FileArea::Local(_) => self.deps.push(Dep::LocalArea),
            FileArea::Generated(_) => (),
        }
    }
}
