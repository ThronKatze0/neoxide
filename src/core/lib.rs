use std::ops::{Deref, DerefMut, Drop};
pub struct SignalPointer<T, E> {
    pub inner: T,
    pub event: E,
}

impl<T, E> Deref for SignalPointer<T, E> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl<T, E> Drop for SignalPointer<T, E> {
    fn drop(&mut self) {
        drop(self)
    }
}
impl<T, E> DerefMut for SignalPointer<T, E> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // TODO: send event
        &mut self.inner
    }
}

impl<T, E> SignalPointer<T, E> {
    fn new(inner: T, event: E) -> SignalPointer<T, E> {
        SignalPointer { inner, event }
    }
}
