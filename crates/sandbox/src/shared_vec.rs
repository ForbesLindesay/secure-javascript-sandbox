use std::sync::{Arc, Mutex};

pub(crate) struct SharedVec<T> {
    inner: Arc<Mutex<Vec<T>>>,
}

impl<T> SharedVec<T> {
    pub fn push(&self, item: T) {
        self.inner
            .lock()
            .expect("lock should never be used twice in the same thread")
            .push(item);
    }
    pub fn take(&self) -> Vec<T> {
        std::mem::take(
            &mut *self
                .inner
                .lock()
                .expect("lock should never be used twice in the same thread"),
        )
    }
}

impl<T> Clone for SharedVec<T> {
    fn clone(&self) -> Self {
        SharedVec {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Default for SharedVec<T> {
    fn default() -> Self {
        SharedVec {
            inner: Arc::default(),
        }
    }
}
