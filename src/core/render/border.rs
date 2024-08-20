use super::BufferBorder;
use std::cmp::min;
use std::fmt::Display;
// TODO: change to unicode
pub const HBORDER: &str = "-";
pub const VBORDER: char = '|';
pub const _DOUBLE_LINEAR_BORDER: char = '=';
pub const CORNER: char = '+';
pub const PADDING: &str = " ";

pub trait PrintBorder {
    fn to_string_border_pad(&self, pad: u16) -> String;
    fn to_string_border(&self, config: &BufferBorder) -> String;
    fn to_string_border_ipad(
        &self,
        corner: char,
        hborder: &str,
        vborder: char,
        lpad: u16,
        rpad: u16,
        tpad: u16,
        dpad: u16,
    ) -> String;
    fn to_string_border_full(
        &self,
        max_height: u16,
        max_width: u16,
        corner: char,
        hborder: &str,
        vborder: char,
        lpad: u16,
        rpad: u16,
        tpad: u16,
        dpad: u16,
    ) -> String;
}

fn push_hborder_full(buf: &mut String, len: usize, corner: char, hborder: &str) {
    buf.push(corner);
    buf.push_str(&hborder.repeat(len - 2));
    buf.push(corner);
    buf.push('\n')
}
fn _push_hborder(buf: &mut String, len: usize) {
    push_hborder_full(buf, len, CORNER, HBORDER)
}

fn push_line_with_vborder_full(buf: &mut String, line: &str, vborder: char) {
    buf.push(vborder);
    buf.push_str(line);
    buf.push(vborder);
    buf.push('\n')
}

fn _push_line_with_vborder(buf: &mut String, line: &str) {
    push_line_with_vborder_full(buf, line, VBORDER)
}

fn push_line_with_vborder_padding(
    buf: &mut String,
    line: &str,
    lpad: usize,
    rpad: usize,
    vborder: char,
) {
    push_line_with_vborder_full(
        buf,
        format!("{}{}{}", &PADDING.repeat(lpad), line, &PADDING.repeat(rpad)).as_str(),
        vborder,
    )
}
impl<T: Display> PrintBorder for T {
    fn to_string_border(&self, config: &BufferBorder) -> String {
        match config {
            BufferBorder::None => self.to_string(),
            BufferBorder::Border {
                corner,
                hborder,
                vborder,
                lpad,
                rpad,
                tpad,
                dpad,
            } => self.to_string_border_ipad(*corner, hborder, *vborder, *lpad, *rpad, *tpad, *dpad),
        }
        // self.to_string_border_pad(1)
    }
    fn to_string_border_pad(&self, pad: u16) -> String {
        self.to_string_border_ipad(CORNER, HBORDER, VBORDER, pad, pad, pad, pad)
    }
    fn to_string_border_ipad(
        &self,
        corner: char,
        hborder: &str,
        vborder: char,
        lpad: u16,
        rpad: u16,
        tpad: u16,
        dpad: u16,
    ) -> String {
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
        self.to_string_border_full(
            max_height, max_width, corner, hborder, vborder, lpad, rpad, tpad, dpad,
        )
    }
    fn to_string_border_full(
        &self,
        max_height: u16,
        max_width: u16,
        corner: char,
        hborder: &str,
        vborder: char,
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
        push_hborder_full(&mut ret, max_width, corner, hborder);

        for _ in 0..tpad {
            push_line_with_vborder_full(&mut ret, &PADDING.repeat(width_without_border), vborder);
        }

        let mut i = 0;
        while i < content.len() {
            if let Some(newline_idx) = content[i..].find('\n') {
                push_line_with_vborder_padding(
                    &mut ret,
                    &content[i..newline_idx],
                    lpad,
                    rpad + (width_without_border - lpad - rpad - (newline_idx - i)),
                    vborder,
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
                    vborder,
                );
                i += end_slice;
            }
        }

        for _ in 0..dpad {
            push_line_with_vborder_full(&mut ret, &PADDING.repeat(width_without_border), vborder);
        }
        push_hborder_full(&mut ret, max_width, corner, hborder);
        ret
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hello_world_no_padding() {
        assert_eq!(
            "+-----+\n|Hello|\n+-----+\n",
            "Hello".to_string_border_full(3, 7, CORNER, HBORDER, VBORDER, 0, 0, 0, 0)
        )
    }

    #[test]
    fn hello_world_no_padding_mean_newlines() {
        assert_eq!(
            "+-----+\n|Hello|\n|World|\n+-----+\n",
            "Hello\nWorld".to_string_border_full(4, 7, CORNER, HBORDER, VBORDER, 0, 0, 0, 0)
        )
    }

    #[test]
    fn hpaddings_eq() {
        assert_eq!(
            "+---+\n| H |\n+---+\n",
            "H".to_string_border_full(3, 5, CORNER, HBORDER, VBORDER, 1, 1, 0, 0)
        );
    }

    #[test]
    fn hpaddings_neq() {
        assert_eq!(
            "+--+\n| H|\n+--+\n",
            "H".to_string_border_full(3, 4, CORNER, HBORDER, VBORDER, 1, 0, 0, 0)
        );
    }

    #[test]
    fn vpaddings_eq() {
        assert_eq!(
            "+-+\n| |\n|H|\n| |\n+-+\n",
            "H".to_string_border_full(3, 3, CORNER, HBORDER, VBORDER, 0, 0, 1, 1)
        );
    }

    #[test]
    fn vpaddings_neq() {
        assert_eq!(
            "+-+\n| |\n|H|\n+-+\n",
            "H".to_string_border_full(3, 3, CORNER, HBORDER, VBORDER, 0, 0, 1, 0)
        );
    }

    #[test]
    fn overflow() {
        assert_eq!(
            "+-----+\n|Hello|\n|World|\n+-----+\n",
            "HelloWorld".to_string_border_full(3, 7, CORNER, HBORDER, VBORDER, 0, 0, 0, 0)
        )
    }

    #[test]
    fn width_detection() {
        assert_eq!(
            "+-----+\n|Hello|\n|foo  |\n+-----+\n",
            "Hello\nfoo".to_string_border_ipad(CORNER, HBORDER, VBORDER, 0, 0, 0, 0)
        );
    }

    #[test]
    fn width_detection_padding() {
        assert_eq!(
            "+------+\n| Hello|\n| foo  |\n+------+\n",
            "Hello\nfoo".to_string_border_ipad(CORNER, HBORDER, VBORDER, 1, 0, 0, 0)
        );
    }

    #[test]
    fn default() {
        assert_eq!(
            "+---+\n|   |\n| H |\n|   |\n+---+\n",
            "H".to_string_border(&BufferBorder::default())
        );
    }
}
