//! # Core Library
//! contains important type definitions/implementations
use std::ops::{Deref, DerefMut, Drop};

/// # SignalPointer<T, E>
/// Simple wrapper, that sends an Event of type 'E' when something is interacting with it
pub struct SignalPointer<T, E> {
    pub inner: T,
    pub deref_event: Option<E>,
    pub drop_event: Option<E>,
    pub deref_mut_event: Option<E>,
}

impl<T, E> Deref for SignalPointer<T, E> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        // TODO: send event
        if let Some(_event) = &self.deref_event {}
        &self.inner
    }
}
impl<T, E> Drop for SignalPointer<T, E> {
    fn drop(&mut self) {
        // TODO: send event
        if let Some(_event) = &self.drop_event {}
        drop(self)
    }
}
impl<T, E> DerefMut for SignalPointer<T, E> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // TODO: send event
        if let Some(_event) = &self.deref_mut_event {}
        &mut self.inner
    }
}

impl<T, E> SignalPointer<T, E> {
    fn new(
        inner: T,
        deref_event: Option<E>,
        drop_event: Option<E>,
        deref_mut_event: Option<E>,
    ) -> SignalPointer<T, E> {
        SignalPointer {
            inner,
            deref_event,
            drop_event,
            deref_mut_event,
        }
    }
}
