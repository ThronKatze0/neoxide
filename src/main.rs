use neoxide::core::editor::motions::{LeftRightMotion, Motion, MotionDirection, UpDownMotion};
use neoxide::core::event_handling::EventCallback;
use neoxide::core::{io, logger, render};
use std::io::{prelude::*, stdin};
use std::ops::{AddAssign, Deref};
use std::time::Duration;
use std::{io::stdout, process::Command};
use tokio::task::JoinHandle;

use crossterm::{
    cursor::{MoveToColumn, MoveToRow},
    event, execute,
    style::Print,
    terminal::{self, ClearType},
};

/// crossterm is a set of "low-level", cross-platform APIs that handle terminal output
/// This could be a potential "frontend", due to it being cross-platform, having *some* way of
/// async, while still letting us do all the UI design
fn demo() -> std::io::Result<()> {
    terminal::enable_raw_mode()?;
    let (w, h) = terminal::size()?;
    execute!(
        stdout(),
        terminal::Clear(ClearType::All),
        MoveToRow(h / 2),
        MoveToColumn(w / 2),
        Print("Hi from Neoxide!")
    )?;
    loop {
        match event::read()? {
            event::Event::Key(_event) => {
                break;
            }
            _ => {}
        }
    }
    execute!(stdout(), terminal::Clear(ClearType::All))?;
    terminal::disable_raw_mode()?;
    Ok(())
}

use neoxide::core::render::{manager::ANSICode, ClientBuffer};
async fn demo_render() -> std::io::Result<()> {
    terminal::enable_raw_mode()?;
    let mut buf1 = ClientBuffer::build(0, true).await.unwrap();
    buf1.set_content(String::from("Test")).await.unwrap();
    let mut buf2 = ClientBuffer::build(0, true).await.unwrap();
    buf2.set_content(String::from("Test 2")).await.unwrap();
    buf1.center().await;
    let mut buf3 = ClientBuffer::build(0, true).await.unwrap();
    buf3.set_content(String::from("Test 3")).await.unwrap();
    buf1.focus().await.unwrap(); // too much stuff depends on it getting called at exactly the
                                 // right time. Needless to say, make it not
    drop(buf1);
    // drop(buf2);
    terminal::disable_raw_mode()?;
    Ok(())
}

use neoxide::core::logger::{log, LogLevel, LOGFILE_PATH};
use neoxide::core::render::manager::{bench, update_cursor_pos, ContentRef};

async fn benchmark(rounds: u32) {
    let mut sum: Duration = Default::default();
    for i in 0..rounds {
        log(LogLevel::Debug, format!("round {}", i + 1).as_str()).await;
        sum.add_assign(bench(10).await);
    }
    println!("Total time: {:.3?}", sum);
    println!("Avg time per round: {:.3?}", sum.div_f64(rounds.into()));
}

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use neoxide::core::input::{self, EvtData, InputConfig, InputEvent};
use std::sync::Arc;
use tokio::sync::Mutex;
async fn editor_demo() {
    let handle = tokio::spawn(input::input_loop(InputConfig {
        bracketed_paste: false,
        focus_change: false,
        mouse_capture: false,
    }));
    let buf = io::open_file("log.neo2").await.unwrap();
    let _ = buf.focus().await;
    input::subscribe(EventCallback::new(
        Arc::new(Box::new(move |evt: Arc<Mutex<_>>| {
            let evt = evt.clone();
            let fut = async move {
                let buf = render::manager::focused().await.unwrap();
                let evt = evt.lock().await;
                if let Event::Key(KeyEvent {
                    code,
                    modifiers: _,
                    kind: KeyEventKind::Press,
                    state: _,
                }) = evt.0
                {
                    let dbr = buf.deref().await;
                    let cursor_position = &dbr.cursor_position();
                    logger::log(LogLevel::Debug, "Fetched buffer content...").await;
                    if let KeyCode::Char(c) = code {
                        let pos = match c {
                            'j' => UpDownMotion.get_new_cursor_position(
                                dbr,
                                cursor_position,
                                MotionDirection::Foward,
                            ),
                            'k' => UpDownMotion.get_new_cursor_position(
                                dbr,
                                cursor_position,
                                MotionDirection::Backward,
                            ),
                            'h' => LeftRightMotion.get_new_cursor_position(
                                dbr,
                                cursor_position,
                                MotionDirection::Backward,
                            ),
                            'l' => LeftRightMotion.get_new_cursor_position(
                                dbr,
                                cursor_position,
                                MotionDirection::Foward,
                            ),
                            _ => todo!(),
                        };
                        logger::log(
                            LogLevel::Normal,
                            format!("New cursor position is {:?}", pos).as_str(),
                        )
                        .await;
                        let mut dbr = buf.deref().await;
                        dbr.set_cursor_pos(pos);
                    }
                    logger::log(LogLevel::Debug, format!("Got keycode: {code}").as_str()).await;
                }
            };
            Box::pin(fut)
        })),
        true,
        InputEvent(Event::Key(KeyEvent::new(
            event::KeyCode::Char(' '), // doesn't matter which char goes here
            KeyModifiers::empty(),
        ))),
    ))
    .await;
    handle.await.unwrap().unwrap();
    drop(buf);
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let _ = Command::new("rm").arg(LOGFILE_PATH).output();
    terminal::enable_raw_mode()?;
    editor_demo().await;
    terminal::disable_raw_mode()?;
    // let test = editor_demo().await.await??;
    // let mut stdin = stdin();
    // let _ = stdin.read(&mut [0u8]).unwrap();
    // let buf = io::open_file("log.neo2").await.unwrap();
    Ok(())
}
