use std::{
    fmt::Display,
    io::{stdout, Write},
};

use crossterm::{
    cursor::MoveTo,
    queue,
    style::Print,
    terminal::{Clear, ClearType},
};

// TODO: change to unicode
const LINEAR_BORDER: char = '-';
const DOUBLE_LINEAR_BORDER: char = '=';
const CORNER_BORDER: char = '+';

trait PrintBorder {
    pub fn to_string_border() -> String;
}

impl PrintBorder for Display {
    fn to_string_border(&self, max_height: u16, max_width: u16) -> String {
        assert!(max_width > 1 && max_height > 1); // TODO: implement actual overflow check
        let ret: [char; max_height * max_width] = [0; max_width * max_height];
        //corners
        ret[0] = ret[max_width - 1] =
            ret[max_height * (max_width - 1)] = ret[max_height * max_width - 1] = CORNER_BORDER;
        let content = self.to_string();

        String::from(ret)
    }
}

fn print_text(text: &str, x: u16, y: u16) {
    queue!(stdout(), MoveTo(x, y), Print(text));
}

fn print_new_line(text: &str, y: u16) {
    print_text(text, 0, y)
}

fn print_lines(text: &str, start_row: u16) {
    text.lines()
        .enumerate()
        .for_each(|i, line| print_new_line(line, start_row + i));
    stdout().flush();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normal() {
        println!("{}", "Hello World".to_string_border());
    }
}
