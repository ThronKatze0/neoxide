use neoxide::core::io;
use std::io::{prelude::*, stdin};
use std::ops::AddAssign;
use std::time::Duration;
use std::{io::stdout, process::Command};

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
    let buf1 = ClientBuffer::build(0, true).await.unwrap();
    buf1.set_content(String::from("Test")).await.unwrap();
    let buf2 = ClientBuffer::build(0, true).await.unwrap();
    buf2.set_content(String::from("Test 2")).await.unwrap();
    buf1.center().await;
    let buf3 = ClientBuffer::build(0, true).await.unwrap();
    buf3.set_content(String::from("Test 3")).await.unwrap();
    drop(buf1);
    drop(buf2);
    terminal::disable_raw_mode()?;
    Ok(())
}

use neoxide::core::logger::{log, LogLevel, LOGFILE_PATH};
use neoxide::core::render::manager::bench;

async fn benchmark(rounds: u32) {
    let mut sum: Duration = Default::default();
    let rounds = 100;
    for i in 0..rounds {
        log(LogLevel::Debug, format!("round {}", i + 1).as_str()).await;
        sum.add_assign(bench(10).await);
    }
    println!("Total time: {:.3?}", sum);
    println!("Avg time per round: {:.3?}", sum.div_f64(rounds.into()));
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let _ = Command::new("rm").arg(LOGFILE_PATH).output();
    let buf = io::open_file("log.neo2").await;
    // bench(10).await;
    // demo_render().await.unwrap();
    // let mut stdin = stdin();
    // let _ = stdin.read(&mut [0u8]).unwrap();
    Ok(())
}
