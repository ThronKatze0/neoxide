use std::{
    cmp::min,
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
const HBORDER: &str = "-";
const VBORDER: char = '|';
const DOUBLE_LINEAR_BORDER: char = '=';
const CORNER: char = '+';
const PADDING: &str = " ";

trait PrintBorder {
    fn to_string_border_pad(&self, pad: u16) -> String;
    fn to_string_border(&self) -> String;
    fn to_string_border_ipad(&self, lpad: u16, rpad: u16, tpad: u16, dpad: u16) -> String;
    fn to_string_border_full(
        &self,
        max_height: u16,
        max_width: u16,
        lpad: u16,
        rpad: u16,
        tpad: u16,
        dpad: u16,
    ) -> String;
}

fn push_hborder(buf: &mut String, len: usize) {
    buf.push(CORNER);
    buf.push_str(&HBORDER.repeat(len - 2));
    buf.push(CORNER);
    buf.push('\n')
}
fn push_line_with_vborder(buf: &mut String, line: &str) {
    buf.push(VBORDER);
    buf.push_str(line);
    buf.push(VBORDER);
    buf.push('\n')
}

fn push_line_with_vborder_padding(buf: &mut String, line: &str, lpad: usize, rpad: usize) {
    push_line_with_vborder(
        buf,
        format!("{}{}{}", &PADDING.repeat(lpad), line, &PADDING.repeat(rpad)).as_str(),
    )
}
impl<T: Display> PrintBorder for T {
    fn to_string_border(&self) -> String {
        self.to_string_border_pad(1)
    }
    fn to_string_border_pad(&self, pad: u16) -> String {
        self.to_string_border_ipad(pad, pad, pad, pad)
    }
    fn to_string_border_ipad(&self, lpad: u16, rpad: u16, tpad: u16, dpad: u16) -> String {
        let max_width = self
            .to_string()
            .split('\n')
            .max_by(|l1, l2| l1.len().cmp(&l2.len()))
            .unwrap_or("")
            .len() as u16
            + 2
            + lpad
            + rpad;
        let max_height = self.to_string().split('\n').count() as u16 + tpad + dpad;
        self.to_string_border_full(max_height, max_width, lpad, rpad, tpad, dpad)
    }
    fn to_string_border_full(
        &self,
        max_height: u16,
        max_width: u16,
        lpad: u16,
        rpad: u16,
        tpad: u16,
        dpad: u16,
    ) -> String {
        // TODO: This error checking is pathetic
        assert!(
            lpad + rpad <= max_width,
            "{lpad} (lpad) + {rpad} (rpad) is greater than specified max_width ({max_width})"
        );
        assert!(
            tpad + dpad <= max_height,
            "{tpad} (tpad) + {dpad} (dpad) is greater than specified max_height ({max_height})"
        );
        assert!(max_width > 1 && max_height > 1);
        let max_height = max_height as usize;
        let max_width = max_width as usize;
        let lpad = lpad as usize;
        let rpad = rpad as usize;
        let tpad = tpad as usize;
        let dpad = dpad as usize;
        let width_without_border = max_width - 2;
        let content = self.to_string();
        let mut ret = String::with_capacity(max_height * (max_width + 1));
        push_hborder(&mut ret, max_width);

        for _ in 0..tpad {
            push_line_with_vborder(&mut ret, &PADDING.repeat(width_without_border));
        }

        let mut i = 0;
        while i < content.len() {
            if let Some(newline_idx) = content[i..].find('\n') {
                push_line_with_vborder_padding(
                    &mut ret,
                    &content[i..newline_idx],
                    lpad,
                    rpad + (width_without_border - lpad - rpad - (newline_idx - i)),
                );
                i += newline_idx + 1; // make sure we skip the newline
            } else {
                let end_slice = min(i + width_without_border, content.len());
                push_line_with_vborder_padding(
                    &mut ret,
                    &content[i..end_slice],
                    lpad,
                    if end_slice + 1 - i + lpad + rpad < width_without_border {
                        width_without_border - (end_slice + lpad - i)
                    } else {
                        rpad
                    },
                );
                i += end_slice;
            }
        }

        for _ in 0..dpad {
            push_line_with_vborder(&mut ret, &PADDING.repeat(width_without_border));
        }
        push_hborder(&mut ret, max_width);
        ret
    }
}

fn print_text(text: &str, x: u16, y: u16) -> std::io::Result<()> {
    queue!(stdout(), MoveTo(x, y), Print(text))?;
    Ok(())
}

fn print_new_line(text: &str, y: u16) -> std::io::Result<()> {
    print_text(text, 0, y)
}

fn print_lines(text: &str, start_row: u16) -> std::io::Result<()> {
    let _ = text
        .lines()
        .enumerate()
        .take_while(|(i, line)| print_new_line(line, start_row + *i as u16).is_ok());
    let _ = stdout().flush();
    Ok(())
}

enum BufferBorder<'a> {
    None,
    Border {
        corner: char,
        hborder: &'a str,
        vborder: char,
    },
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
    fn new(offx: u16, offy: u16, width: u16, height: u16) -> Buffer<'a, T> {
        Buffer {
            offx,
            offy,
            width,
            height,
            layer: 0,
            border: BufferBorder::Border {
                corner: CORNER,
                hborder: HBORDER,
                vborder: VBORDER,
            },
            children: Vec::with_capacity(STANDARD_BUFFER_CHILDREN_SIZE),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hello_world_no_padding() {
        assert_eq!(
            "+-----+\n|Hello|\n+-----+\n",
            "Hello".to_string_border_full(3, 7, 0, 0, 0, 0)
        )
    }

    #[test]
    fn hello_world_no_padding_mean_newlines() {
        assert_eq!(
            "+-----+\n|Hello|\n|World|\n+-----+\n",
            "Hello\nWorld".to_string_border_full(4, 7, 0, 0, 0, 0)
        )
    }

    #[test]
    fn hpaddings_eq() {
        assert_eq!(
            "+---+\n| H |\n+---+\n",
            "H".to_string_border_full(3, 5, 1, 1, 0, 0)
        );
    }

    #[test]
    fn hpaddings_neq() {
        assert_eq!(
            "+--+\n| H|\n+--+\n",
            "H".to_string_border_full(3, 4, 1, 0, 0, 0)
        );
    }

    #[test]
    fn vpaddings_eq() {
        assert_eq!(
            "+-+\n| |\n|H|\n| |\n+-+\n",
            "H".to_string_border_full(3, 3, 0, 0, 1, 1)
        );
    }

    #[test]
    fn vpaddings_neq() {
        assert_eq!(
            "+-+\n| |\n|H|\n+-+\n",
            "H".to_string_border_full(3, 3, 0, 0, 1, 0)
        );
    }

    #[test]
    fn overflow() {
        assert_eq!(
            "+-----+\n|Hello|\n|World|\n+-----+\n",
            "HelloWorld".to_string_border_full(3, 7, 0, 0, 0, 0)
        )
    }

    #[test]
    fn width_detection() {
        assert_eq!(
            "+-----+\n|Hello|\n|foo  |\n+-----+\n",
            "Hello\nfoo".to_string_border_ipad(0, 0, 0, 0)
        );
    }

    #[test]
    fn width_detection_padding() {
        assert_eq!(
            "+------+\n| Hello|\n| foo  |\n+------+\n",
            "Hello\nfoo".to_string_border_ipad(1, 0, 0, 0)
        );
    }

    #[test]
    fn default() {
        assert_eq!(
            "+---+\n|   |\n| H |\n|   |\n+---+\n",
            "H".to_string_border()
        );
    }
}
