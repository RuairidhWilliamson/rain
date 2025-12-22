use std::{collections::HashMap, sync::Arc};

use crate::{
    ast::NodeId,
    ir::{IrModule, ModuleId},
    local_span::LocalSpan,
    runner::{
        dep_list::DepList,
        error::{ErrorTrace, RunnerError, Throwing},
        value::Value,
    },
};

use super::dep::Dep;

pub struct Cx<'a> {
    pub module: &'a Arc<IrModule>,
    pub call_depth: usize,
    pub locals: HashMap<&'a str, Value>,
    pub captures: Vec<Arc<HashMap<String, Value>>>,
    pub args: HashMap<&'a str, Value>,
    pub deps: DepList,
    pub previous_line: Option<Value>,
    pub stacktrace: Vec<StacktraceEntry>,
}

impl<'a> Cx<'a> {
    #[must_use]
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
            deps: DepList::new(),
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

    #[must_use]
    pub fn callee(
        &self,
        module: &'a Arc<IrModule>,
        args: HashMap<&'a str, Value>,
        captures: &Arc<HashMap<String, Value>>,
        ste: StacktraceEntry,
    ) -> Self {
        let mut st = self.stacktrace.clone();
        st.push(ste);
        let mut callee = Cx::new(module, self.call_depth + 1, args, st);
        callee.captures.clone_from(&self.captures);
        callee.captures.push(Arc::clone(captures));
        callee
    }

    pub fn propagate_deps(&mut self, callee_deps: DepList) {
        self.deps.extend(
            callee_deps
                .into_iter()
                .filter(Dep::is_propogated_in_closure),
        );
    }
}

#[derive(Debug, Clone)]
pub struct StacktraceEntry {
    pub m: ModuleId,
    pub n: NodeId,
}
