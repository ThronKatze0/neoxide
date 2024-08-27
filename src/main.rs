use futures::{future::join, join};
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

use neoxide::core::render::ClientBuffer;
async fn demo_render() -> std::io::Result<()> {
    // terminal::enable_raw_mode()?;
    // stdout().execute(terminal::Clear(ClearType::All))?;
    // let mut buf1: Buffer<_> = Buffer::default("buf1");
    // buf1.children.push("Test");
    //
    // let mut buf2: Buffer<_> = Buffer::default("buf2");
    // buf2.offx = 10;
    // buf2.offy = 10;
    // buf2.children.push("Test 2");
    //
    // buf1.render().await?; // but our wise friend BUFMAN_SINGLETON now watches over us naive beings
    //                       // and makes sure we don't overwrite eachother's framebuffer
    // buf2.render().await?;
    // terminal::disable_raw_mode()?;
    terminal::enable_raw_mode()?;
    let buf1 = ClientBuffer::build(0, "buf1").await.unwrap();
    println!("Added buffer!");
    buf1.set_content(String::from("Test")).await.unwrap();
    let buf2 = ClientBuffer::build(0, "buf2").await.unwrap();
    buf2.set_content(String::from("Test 2")).await.unwrap();
    let buf3 = ClientBuffer::build(0, "buf3").await.unwrap();
    buf3.set_content(String::from("Test 3")).await.unwrap();
    terminal::disable_raw_mode()?;
    Ok(())
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    demo_render().await.unwrap();
    loop {}
    Ok(())
}
