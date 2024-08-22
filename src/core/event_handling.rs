use futures::future::join_all;
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
type EventCallbackFunctionType<D> = Arc<
    Box<dyn Fn(Arc<Mutex<D>>) -> Pin<Box<dyn Future<Output = ()> + Send + Sync>> + Send + Sync>,
>;
pub struct EventCallback<E, D> {
    callback: EventCallbackFunctionType<D>,
    permanent: bool,
    event: E,
}

impl<E, D> EventCallback<E, D> {
    fn new(callback: EventCallbackFunctionType<D>, permanent: bool, event: E) -> Self {
        EventCallback {
            callback,
            permanent,
            event,
        }
    }
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
            subscriptions: Mutex::new({
                let mut temp = Vec::with_capacity(E::COUNT);
                for i in 0..E::COUNT {
                    temp.push(HashMap::new());
                }
                temp
            }),
        }
    }

    pub async fn subscribe(&self, event_callback: EventCallback<E, D>) -> u32 {
        let mut lock = self.subscriptions.lock().await;
        let enum_idx = get_enum_position(event_callback.event).await;
        let callback_map = lock
            .get_mut(enum_idx as usize)
            .expect(&format!("unsafe code not so good (sub;{enum_idx})"));
        let id: u32 = rand::thread_rng().gen();
        callback_map.insert(id, event_callback);
        id
    }

    pub async fn dispatch(&self, event: E, data: D)
    where
        E: Send + 'static,
    {
        let lock = self.subscriptions.lock().await;
        let data = Arc::new(Mutex::new(data));
        let callback_map = lock
            .get(get_enum_position(event).await as usize)
            .expect("unsafe code not so good (dispatch)");
        // I think the rustaceans consider this to be more idiomatic
        let futs: Vec<_> = callback_map
            .into_iter()
            .map(|(_, event_callback)| {
                let callback = Arc::clone(&event_callback.callback);
                let callback_data = Arc::clone(&data);
                (callback)(callback_data)
            })
            .collect();
        // for (_, event_callback) in callback_map {
        //     let callback = Arc::clone(&event_callback.callback);
        //     let callback_data = Arc::clone(&data);
        //     // task::spawn returns a join handle, that can be used to wait until that task has
        //     // finished. This doesn't do that, meaning this function will return before all
        //     // callbacks have finished and therefore breaking the test case (Side note on test case: I rewrote it to only use one task and now the test case works without me waiting for it???). Of course, it is
        //     // debatable wether this is the intended behavior or not.
        //     // also isn't it kinda wasteful to make a new task for every callback?
        //     // tokio::task::spawn(async move {
        //     //     (callback)(callback_data).await;
        //     // });
        //     // futs.push((callback)(callback_data));
        // }
        // Do something with this, if you'd like
        let join_handle = tokio::task::spawn(async move {
            join_all(futs.into_iter()).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_unsafe_code() {
        assert_eq!(get_enum_position(DemoEvent::Quit).await, 2);
    }

    static mut TEST_SUCCESSFUL: bool = false;

    #[tokio::test]
    async fn test_event_handler() {
        let event_handler = EventHandler::<DemoEvent, DemoEventData>::new().await;
        let event_callback = EventCallback::new(
            Arc::new(Box::new(|_| {
                unsafe {
                    TEST_SUCCESSFUL = true;
                }
                Box::pin(async { () })
            })),
            false,
            DemoEvent::NormalEnter,
        );
        event_handler.subscribe(event_callback).await;
        event_handler
            .dispatch(
                DemoEvent::NormalEnter,
                DemoEventData {
                    cursor_position: 1,
                    cool_string: "".to_string(),
                },
            )
            .await;
        unsafe {
            assert!(TEST_SUCCESSFUL);
        }
    }
}
