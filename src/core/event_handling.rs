use rand::Rng;
use std::future::Future;
use std::hash::Hash;
use std::pin::Pin;
use std::sync::Arc;
use std::{collections::HashMap, ops::AsyncFn};
use strum::EnumCount;
use strum_macros::EnumCount as EnumCountMacro;
use tokio;
use tokio::pin;
use tokio::sync::Mutex;

#[derive(Clone, Copy, PartialEq, Hash, EnumCountMacro)]
#[repr(u8)]
enum DemoEvent {
    InsertEnter,
    NormalEnter,
    Quit,
}

async fn get_enum_position<T>(enum_type: T) -> u8 {
    let ptr = &enum_type as *const _ as *const u8;
    let size = std::mem::size_of_val(&enum_type);
    let bytes: &[u8] = unsafe { std::slice::from_raw_parts(ptr, size) };
    bytes[0]
}

struct DemoEventData {
    cursor_position: i32,
    cool_string: String,
}

pub struct EventCallback<E, D> {
    callback: Arc<
        Box<dyn Fn(Arc<Mutex<D>>) -> Pin<Box<dyn Future<Output = ()> + Send + Sync>> + Send + Sync>,
    >,
    permanent: bool,
    event: E,
}

pub struct EventHandler<E, D>
where
    E: EnumCount + Clone + Copy + Send,
    D: Send,
{
    subscriptions: Mutex<Vec<HashMap<u32, EventCallback<E, D>>>>,
}

impl<E, D> EventHandler<E, D>
where
    E: EnumCount + Clone + Copy + Send + 'static,
    D: Send + 'static,
{
    pub async fn new() -> EventHandler<E, D> {
        EventHandler {
            subscriptions: Mutex::new(Vec::with_capacity(E::COUNT)),
        }
    }

    pub async fn subscribe(&self, event_callback: EventCallback<E, D>) -> u32 {
        let mut lock = self.subscriptions.lock().await;
        let callback_map = lock
            .get_mut(get_enum_position(event_callback.event).await as usize)
            .expect("unsafe code not so good");
        let id: u32 = rand::thread_rng().gen();
        callback_map.insert(id, event_callback);
        id
    }

    pub async fn dispatch(&self, event: E, data: D)
    where
        E: Send + 'static,
        D: Send + 'static,
    {
        let lock = self.subscriptions.lock().await;
        let data = Arc::new(Mutex::new(data));
        let callback_map = lock
            .get(get_enum_position(event).await as usize)
            .expect("unsafe code not so good");
        for (_, event_callback) in callback_map {
            let callback = Arc::clone(&event_callback.callback);
            let callback_data = Arc::clone(&data);
            tokio::task::spawn(async move {
                (callback)(callback_data).await;
            });
        }
    }
}

mod tests {
    use super::*;

    #[tokio::test]
    async fn test_unsafe_code() {
        assert_eq!(get_enum_position(DemoEvent::Quit).await, 2);
    }

    #[tokio::test]
    async fn test_event_handler() {
        let event_handler = EventHandler::<DemoEvent,DemoEventData>::new().await;
        event_handler.subscribe();
    }
}
