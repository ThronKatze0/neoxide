use futures::{future::join, join};
use neoxide::core::{lib::SignalPointerEventDispatcher, render::Buffer};
use std::io::stdout;
use strum::EnumCount;
use strum_macros::{EnumCount as EnumCountMacro, EnumIter};

use crossterm::{
    cursor::{MoveToColumn, MoveToRow},
    event, execute,
    style::Print,
    terminal::{self, ClearType},
    ExecutableCommand,
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

async fn demo_render() -> std::io::Result<()> {
    terminal::enable_raw_mode()?;
    stdout().execute(terminal::Clear(ClearType::All))?;
    let mut buf1: Buffer<_> = Buffer::default();
    // TODO: Vincent, remind me of how stupid I am for not implementing auto-width-detection
    buf1.width = 8;
    buf1.height = 5;
    buf1.children.push("Test");

    let mut buf2: Buffer<_> = Buffer::default();
    // TODO: Vincent, remind me of how stupid I am for not implementing auto-width-detection
    buf2.height = 5;
    buf2.width = 10;
    buf2.offx = 10;
    buf2.offy = 10;
    buf2.children.push("Test 2");

    buf1.render().await?; // but our wise friend BUFMAN_SINGLETON now watches over us naive beings
                          // and makes sure we don't overwrite eachother's framebuffer
    buf2.render().await?;
    terminal::disable_raw_mode()?;
    Ok(())
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    demo_render().await?;
    Ok(())
}
