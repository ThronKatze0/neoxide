use std::{
    fmt::Display,
    io::{stdout, Write},
};

use crossterm::{cursor::MoveTo, queue, style::Print};

mod border;

use border::{PrintBorder, CORNER, HBORDER, VBORDER};

fn print_text(text: &str, x: u16, y: u16) -> std::io::Result<()> {
    queue!(stdout(), MoveTo(x, y), Print(text))?;
    Ok(())
}

fn print_lines(text: &str, start_row: u16, padding_left: u16) -> std::io::Result<()> {
    let _ = text
        .lines()
        .enumerate()
        .take_while(|(i, line)| print_text(line, padding_left, start_row + *i as u16).is_ok());
    let _ = stdout().flush();
    Ok(())
}

enum BufferBorder<'a> {
    None,
    Border {
        corner: char,
        hborder: &'a str,
        vborder: char,
        lpad: u16,
        rpad: u16,
        tpad: u16,
        dpad: u16,
    },
}

impl<'a> BufferBorder<'a> {
    fn default() -> BufferBorder<'a> {
        BufferBorder::Border {
            corner: CORNER,
            hborder: HBORDER,
            vborder: VBORDER,
            lpad: 1,
            rpad: 1,
            tpad: 1,
            dpad: 1,
        }
    }
}

struct Buffer<'a, T: Display> {
    pub offx: u16,
    pub offy: u16,
    pub width: u16,
    pub height: u16,
    pub layer: u8,
    border: BufferBorder<'a>,
    pub children: Vec<T>,
}

const STANDARD_BUFFER_CHILDREN_SIZE: usize = 5;
impl<'a, T: Display> Buffer<'a, T> {
    pub fn new(offx: u16, offy: u16, width: u16, height: u16) -> Buffer<'a, T> {
        Buffer {
            offx,
            offy,
            width,
            height,
            layer: 0,
            border: BufferBorder::default(),
            children: Vec::with_capacity(STANDARD_BUFFER_CHILDREN_SIZE),
        }
    }

    async fn render(&self) -> std::io::Result<()> {
        print_lines(&self.to_string_border(&self.border), self.offy, self.offx)?;
        Ok(())
    }
}

impl<'a, T: Display> Display for Buffer<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut ret = String::new();
        for child in self.children.iter() {
            ret.push_str(&child.to_string());
        }
        write!(f, "{ret}")?;
        Ok(())
    }
}
