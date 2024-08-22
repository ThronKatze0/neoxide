//! # Core Library
//! contains important type definitions/implementations
use futures::Future;
use std::ops::Deref;
use std::ops::DerefMut;
use std::pin::Pin;
use std::sync::Arc;
use strum::EnumCount;

use crate::core::event_handling::EventHandler;
use std::sync::mpsc;

#[derive(Clone)]
struct SignalPointerName(&'static str);
unsafe impl Send for SignalPointerName {}

/// # SignalPointer<T, E>
/// Simple wrapper, that sends an Event of type 'E' when something is interacting with it
/// Note: E and A must be of ``'static`` lifetime, since the borrow checker is too stoopid to know
/// that it they will live long enough anyways
struct SignalPointer<T, E>
where
    E: EnumCount + Copy + Send + 'static,
{
    inner: T,
    name: SignalPointerName,
    handler: Arc<EventHandler<E, SignalPointerName>>,
    sender: Speds,
    deref_event: Option<E>,
    drop_event: Option<E>,
    deref_mut_event: Option<E>,
}

impl<T, E> DerefMut for SignalPointer<T, E>
where
    E: EnumCount + Copy + Send + 'static,
{
    fn deref_mut(&mut self) -> &mut T {
        self.send_event(&self.deref_mut_event);
        &mut self.inner
    }
}

impl<T, E> Deref for SignalPointer<T, E>
where
    E: EnumCount + Copy + Send + 'static,
{
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.send_event(&self.deref_event);
        &self.inner
    }
}

impl<T, E> SignalPointer<T, E>
where
    E: EnumCount + Copy + Send + 'static,
{
    fn send_event(&self, event: &Option<E>) {
        if let Some(event) = event {
            let event = event.clone();
            let handler = Arc::clone(&self.handler);
            let name = self.name.clone();
            let fut = Box::pin(async move { handler.dispatch(event, name).await });
            self.sender
                .send(fut)
                .expect("Event Dispatcher Task offline!"); // should never happen
        }
    }
    pub fn new(
        inner: T,
        name: &'static str,
        handler: Arc<EventHandler<E, SignalPointerName>>,
        deref_event: Option<E>,
        drop_event: Option<E>,
        deref_mut_event: Option<E>,
    ) -> SignalPointer<T, E> {
        SignalPointer {
            inner,
            name: SignalPointerName(name),
            handler,
            sender: unsafe { SPED.get_sender() },
            deref_event,
            drop_event,
            deref_mut_event,
        }
    }

    pub async fn deref(&self) -> &T {
        self.send_event(&self.deref_event);
        &self.inner
    }
    pub async fn deref_mut(&mut self) -> &mut T {
        self.send_event(&self.deref_mut_event);
        &mut self.inner
    }
}

// speds = SignalPointerEventDispatcherSender
type SpedFuture = Pin<Box<dyn Future<Output = ()>>>;
type Speds = mpsc::Sender<SpedFuture>;
type Spedr = mpsc::Receiver<SpedFuture>;
struct SignalPointerEventDispatcher {
    queue: Spedr,
    sender: Speds,
}

use std::sync::LazyLock;
static mut SPED: LazyLock<SignalPointerEventDispatcher> =
    LazyLock::new(|| SignalPointerEventDispatcher::new());
impl SignalPointerEventDispatcher {
    pub async fn init() {
        unsafe {
            while let Ok(val) = SPED.queue.recv() {
                val.await; //.expect("If you're seeing this message, then someone panicked while using the event_stream, but it wasn't us.");
            }
        }
    }
    fn new() -> Self {
        let (sender, queue) = mpsc::channel();

        SignalPointerEventDispatcher { queue, sender }
    }
    fn get_sender(&self) -> Speds {
        self.sender.clone()
    }
}
