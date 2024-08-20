use std::io::stdout;

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

#[tokio::main]
async fn main() {}
