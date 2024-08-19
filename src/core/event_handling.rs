use rand::Rng;
use std::hash::Hash;
use std::{collections::HashMap, ops::AsyncFn};
use strum_macros::EnumDiscriminants;

#[derive(EnumDiscriminants, PartialEq, Hash)]
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
    A: AsyncFn(E) -> Result<(), &'static str>,
{
    subscriptions: HashMap<E, HashMap<u32, EventCallback<E, A>>>,
}

impl<E, A> EventHandler<E, A>
where
    E: Eq + Hash,
    A: AsyncFn(E) -> Result<(), &'static str>,
{
    async fn new() -> EventHandler<E, A> {
        EventHandler {
            subscriptions: HashMap::new(),
        }
    }

    async fn subscribe(&mut self, event: E, callback: EventCallback<E, A>) -> u32 {
        let callback_map = self.subscriptions.entry(event).or_insert(HashMap::new());
        let id: u32 = rand::thread_rng().gen();
        callback_map.insert(id, callback);
        id
    }

    async fn unsubscribe(&mut self, event: E, id: u32) -> Result<(), &'static str> {
        let callback_map = match self.subscriptions.get_mut(&event) {
            Some(callback_map) => callback_map,
            None => return Err("No callbacks for event"),
        };
        match callback_map.remove(&id) {
            Some(_) => return Ok(()),
            None => return Err("No callback with this id found"),
        }
    }
}
