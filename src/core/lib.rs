//! # Core Library
//! contains important type definitions/implementations
use futures::Future;
use std::ops::Deref;
use std::ops::DerefMut;
use std::pin::Pin;
use std::sync::Arc;
use strum::EnumCount;
use tokio::sync::Mutex;

use crate::core::event_handling::EventHandler;
use std::sync::mpsc;

#[derive(Clone)]
struct SignalPointerName(&'static str);
unsafe impl Send for SignalPointerName {}

/// # SignalPointer<T, E>
/// Simple wrapper, that sends an Event of type 'E' when something is interacting with it
/// Note: E and A must be of ``'static`` lifetime, since the borrow checker is too stoopid to know
/// that it they will live long enough anyways
pub struct SignalPointer<T, E, D>
where
    E: EnumCount + Copy + Send + 'static,
    D: Send + 'static,
{
    pub inner: T,
    name: SignalPointerName,
    handler: Arc<EventHandler<E, D>>,
    sender: Speds,
    pub deref_event: Option<(E, Arc<Mutex<D>>)>,
    pub deref_mut_event: Option<(E, Arc<Mutex<D>>)>,
}

impl<T, E, D> DerefMut for SignalPointer<T, E, D>
where
    E: EnumCount + Copy + Send + 'static,
    D: Send + 'static + Clone,
{
    fn deref_mut(&mut self) -> &mut T {
        self.send_event(&self.deref_mut_event);
        &mut self.inner
    }
}

impl<T, E, D> Deref for SignalPointer<T, E, D>
where
    E: EnumCount + Copy + Send + 'static,
    D: Send + 'static + Clone,
{
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.send_event(&self.deref_event);
        &self.inner
    }
}

impl<T, E, D> SignalPointer<T, E, D>
where
    E: EnumCount + Copy + Send + 'static,
    D: Send + 'static,
{
    fn send_event(&self, event: &Option<(E, Arc<Mutex<D>>)>) {
        if let Some((event, data)) = event {
            let event = event.clone();
            let handler = Arc::clone(&self.handler);
            let data = Arc::clone(&data);
            let fut = Box::pin(async move { handler.dispatch(event, data).await });
            self.sender
                .send(fut)
                .expect("Event Dispatcher Task offline!"); // should never happen
        }
    }
    pub fn new(
        inner: T,
        name: &'static str,
        handler: Arc<EventHandler<E, D>>,
        deref_event: Option<(E, D)>,
        deref_mut_event: Option<(E, D)>,
    ) -> SignalPointer<T, E, D> {
        let deref_event = if let Some((event, data)) = deref_event {
            Some((event, Arc::new(Mutex::new(data))))
        } else {
            None
        };
        let deref_mut_event = if let Some((event, data)) = deref_mut_event {
            Some((event, Arc::new(Mutex::new(data))))
        } else {
            None
        };
        SignalPointer {
            inner,
            name: SignalPointerName(name),
            handler,
            sender: unsafe { SPED.get_sender() },
            deref_event,
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
pub struct SignalPointerEventDispatcher {
    queue: Spedr,
    sender: Speds,
}

use std::sync::LazyLock;
static mut SPED: LazyLock<SignalPointerEventDispatcher> =
    LazyLock::new(|| SignalPointerEventDispatcher::new());
unsafe impl Send for SignalPointerEventDispatcher {}
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
