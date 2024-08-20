use rand::Rng;
use std::hash::Hash;
use std::sync::{Arc, Mutex, PoisonError};
use std::{collections::HashMap, ops::AsyncFn};
use strum::EnumCount;
use strum_macros::EnumCount as EnumCountMacro;
use tokio;

#[derive(PartialEq, Hash, EnumCountMacro)]
#[repr(u8)]
enum DemoEvent {
    InsertEnter(String),
    NormalEnter(i32),
    Quit,
}

struct EventCallback<E, A>
where
    A: AsyncFn(E) -> Result<(), &'static str>,
{
    callback: A,
    permanent: bool,
    event: E,
}

pub struct EventHandler<E, A>
where
    E: Eq + Hash + EnumCount,
    A: AsyncFn(E) -> Result<(), &'static str>,
{
    subscriptions: Arc<Mutex<Vec<HashMap<u32, EventCallback<E, A>>>>>,
}

impl<E, A> EventHandler<E, A>
where
    E: Eq + Hash + EnumCount,
    A: AsyncFn(E) -> Result<(), &'static str>,
{
    async fn new() -> EventHandler<E, A> {
        EventHandler {
            subscriptions: Arc::new(Mutex::new(Vec::with_capacity(E::COUNT))),
        }
    }

    async fn get_enum_position<T>(enum_type: T) -> u8 {
        let ptr = &enum_type as *const _ as *const u8;
        let size = std::mem::size_of_val(&enum_type);
        let bytes: &[u8] = unsafe { std::slice::from_raw_parts(ptr, size) };
        bytes[0]
    }

    async fn subscribe(&self, event: E, callback: EventCallback<E, A>) -> Result<u32, String> {
        let subscriptions = Arc::clone(&self.subscriptions);

        let mut lock = subscriptions.lock().map_err(|e| e.to_string())?;
        let callback_map = lock
            .get_mut(Self::get_enum_position(event).await as usize)
            .expect("unsafe stuff went really wrong");
        let id: u32 = rand::thread_rng().gen();
        callback_map.insert(id, callback);
        Ok(id)
    }

    async fn unsubscribe(&mut self, event: E, id: u32) -> Result<(), String> {
        let subscriptions = Arc::clone(&self.subscriptions);
        let mut lock = subscriptions.lock().map_err(|e| e.to_string())?;
        let callback_map = lock
            .get_mut(Self::get_enum_position(event).await as usize)
            .expect("unsafe stuff went really wrong");
        match callback_map.remove(&id) {
            Some(_) => return Ok(()),
            None => return Err("No callback with this id found".to_string()),
        }
    }

    async fn dispatch(&self, event: E) -> Result<(), String> {
        let subscriptions = Arc::clone(&self.subscriptions);
        let mut lock = subscriptions.lock().map_err(|e| e.to_string())?;
        let callback_map = lock
            .get_mut(Self::get_enum_position(event).await as usize)
            .expect("unsafe stuff went really wrong");
        callback_map.iter().for_each(|event_callback| todo!());
        Ok(())
    }
}
