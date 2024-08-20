use std::io::stdout;
use strum::EnumCount;
use strum_macros::{EnumCount as EnumCountMacro, EnumIter};

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

#[derive(EnumCountMacro)]
enum Enum {
    Unit,
    Tuple(bool),
    Struct { a: bool, test: (u8, i32) },
}

impl Enum {
    fn discriminant(&self) -> u8 {
        // SAFETY: Because `Self` is marked `repr(u8)`, its layout is a `repr(C)` `union`
        // between `repr(C)` structs, each of which has the `u8` discriminant as its first
        // field, so we can read the discriminant without offsetting the pointer.
        unsafe { *<*const _>::from(self).cast::<u8>() }
    }
}
fn main() {
    let test = Enum::Struct {
        a: true,
        test: (34, 53),
    };
    let ptr = &test as *const _ as *const u8;
    let size = std::mem::size_of_val(&test);
    let bytes: &[u8] = unsafe { std::slice::from_raw_parts(ptr, size) };
    let number: u8 = bytes[0];
    println!("{}", number);
    get_enum_count(test);
}

fn get_enum_count<E: EnumCount>(_enum: E) {
    println!("{}", E::COUNT);
}
