use std::io::{stdout, Write};

use crossterm::{
    cursor::MoveTo,
    queue,
    style::Print,
    terminal::{Clear, ClearType},
};

fn print_text(text: &str, x: u16, y: u16) {
    queue!(stdout(), MoveTo(x, y), Print(text));
}

fn print_new_line(text: &str, y: u16) {
    print_text(text, 0, y)
}

pub fn print_lines(text: &str, start_row: u16) {
     text.lines()
         .enumerate()
         .for_each(|i, line| print_new_line(line, start_row + i));
     stdout().flush();
}
