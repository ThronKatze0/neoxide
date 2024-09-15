use crossterm::event::{poll, KeyEvent, KeyModifiers};
use std::future::Future;
use std::io::{stdout, Result as IoResult, Write};
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;
use tokio::sync::Mutex;

use once_cell::sync::Lazy;
use strum::EnumCount;

use super::event_handling::{EventCallback, EventHandler};
use super::logger::{self, LogLevel};
use super::render;
use crossterm::{
    event::{
        read, DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
        EnableFocusChange, EnableMouseCapture, Event,
    },
    Command, QueueableCommand,
};

pub struct InputEvent(pub Event);
impl EnumCount for InputEvent {
    const COUNT: usize = 6;
}
impl Clone for InputEvent {
    fn clone(&self) -> Self {
        InputEvent(self.0.clone())
    }
}

pub struct EvtData(pub Event);
static INPUT_EVH: Lazy<EventHandler<InputEvent, EvtData>> = Lazy::new(EventHandler::new);

pub async fn subscribe(evcb: EventCallback<InputEvent, EvtData>) -> u32 {
    INPUT_EVH.subscribe(evcb).await
}
pub async fn unsub(event: InputEvent, id: u32) -> Result<(), &'static str> {
    INPUT_EVH.unsubscribe(event, id).await
}

struct InputFuture;
impl Future for InputFuture {
    type Output = IoResult<Event>;
    fn poll(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        // this is in the docs, though it might upset tokio
        if poll(Duration::from_millis(100))? {
            Poll::Ready(read())
        } else {
            Poll::Pending
        }
    }
}

fn get_next_event() -> InputFuture {
    InputFuture
}

pub struct InputConfig {
    pub bracketed_paste: bool,
    pub focus_change: bool,
    pub mouse_capture: bool,
}

fn set_opt(opt: bool, enable_com: impl Command, disable_com: impl Command) -> IoResult<()> {
    if opt {
        stdout().queue(enable_com)?;
    } else {
        stdout().queue(disable_com)?;
    }
    Ok(())
}

use crossterm::event::KeyCode;

/// The main loop, that will transmit all InputEvents over the Event Handling system.
/// This function needs to be only called once on initialization (maybe I should write some code to
/// prevent calling it multiple times) and should live in it's own tokio task. This function
/// follows the fail-fast principle, and returns on the first IO error it sees, which means that
/// once crossterm breaks, you can't send keypresses etc. anymore
/// All Events are directly transferred to the dedicated Event Handler, provided through a
/// newtype pattern, which implements the Clone- and EnumCount traits for the events
pub async fn input_loop(config: InputConfig) -> IoResult<()> {
    set_opt(
        config.bracketed_paste,
        EnableBracketedPaste,
        DisableBracketedPaste,
    )?;
    set_opt(config.focus_change, EnableFocusChange, DisableFocusChange)?;
    set_opt(
        config.mouse_capture,
        EnableMouseCapture,
        DisableMouseCapture,
    )?;
    stdout().flush()?;

    loop {
        // NOTE: look into streams for this
        let evt = read()?;
        let evt_data = Arc::new(Mutex::new(EvtData(evt.clone())));
        logger::log(LogLevel::Normal, format!("Sending event: {evt:?}").as_str()).await;
        match evt {
            Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }) => break,
            Event::Key(KeyEvent {
                code: KeyCode::Char(c),
                ..
            }) if c >= '0' && c <= '9' => {}
            Event::Resize(_, _) => render::manager::dispatch_resize().await,
            evt => {
                let evt = InputEvent(evt);
                INPUT_EVH.dispatch(evt, evt_data).await;
            }
        }
    }
    Ok(())
}
