use std::sync::{Arc, Once};

#[derive(Clone)]
pub struct Cancellation(Arc<Once>);

impl Default for Cancellation {
    fn default() -> Self {
        Self::new()
    }
}

impl Cancellation {
    pub fn new() -> Self {
        Self(Arc::new(Once::new()))
    }

    pub fn cancel(&self) {
        self.0.call_once(|| {});
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.is_completed()
    }

    pub fn wait(&self) {
        self.0.wait();
    }
}
