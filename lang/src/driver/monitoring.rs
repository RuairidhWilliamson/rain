use serde::{Deserialize, Serialize};

use crate::runner::internal::InternalFunction;

pub trait MonitoringTrait {
    fn enter_call(&self, _s: &Call) {}
    fn exit_call(&self, _s: &Call) {}

    #[must_use]
    fn call_guard(&self, call: Call) -> CallGuard<'_>
    where
        Self: Sized,
    {
        self.enter_call(&call);
        CallGuard { driver: self, call }
    }
}

pub struct CallGuard<'a> {
    driver: &'a dyn MonitoringTrait,
    call: Call,
}

impl Drop for CallGuard<'_> {
    fn drop(&mut self) {
        self.driver.exit_call(&self.call);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Call {
    Declaration(String),
    Closure,
    Internal(InternalFunction),
    Custom(String),
}
