/// Types that implement [`Alias`] are cheap to clone because their clone points to the same underlying data
pub trait Alias: Clone {
    /// Clone the reference without performing an expensive [`Clone::clone`] on the underlying data
    #[must_use]
    fn alias(&self) -> Self {
        Self::clone(self)
    }
}

impl<T> Alias for std::rc::Rc<T> {}
impl<T> Alias for std::rc::Weak<T> {}
impl<T> Alias for std::sync::Arc<T> {}
impl<T> Alias for std::sync::Weak<T> {}
