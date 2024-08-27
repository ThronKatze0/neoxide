use super::manager::BufferBorder;
use std::cmp::min;
use std::fmt::Display;
// TODO: change to unicode
pub const HBORDER: &str = "-";
pub const VBORDER: char = '|';
pub const _DOUBLE_LINEAR_BORDER: char = '=';
pub const CORNER: char = '+';
pub const PADDING: &str = " ";

pub trait PrintBorder {
    // fn to_string_border_pad(&self, pad: u16) -> String;
    fn to_string_border(&self, config: Option<&BufferBorder>) -> String;
    fn to_string_border_full_with_struct(
        &self,
        max_width: u16,
        max_height: u16,
        config: Option<&BufferBorder>,
    ) -> String;
    fn get_auto_size(&self, lpad: u16, rpad: u16, tpad: u16, dpad: u16) -> (u16, u16);
    fn to_string_border_ipad(
        &self,
        show_borders: (bool, bool, bool, bool),
        corner: char,
        hborder: &'static str,
        vborder: char,
        lpad: u16,
        rpad: u16,
        tpad: u16,
        dpad: u16,
    ) -> String;

    #[deprecated(
        since = "0.1.0",
        note = "please use to_string_border_full_with_struct instead"
    )]
    fn to_string_border_full(
        &self,
        max_height: u16,
        max_width: u16,
        border_shown: (bool, bool, bool, bool),
        corner: char,
        hborder: &'static str,
        vborder: char,
        lpad: u16,
        rpad: u16,
        tpad: u16,
        dpad: u16,
    ) -> String;
}

fn push_hborder_full(
    buf: &mut String,
    len: usize,
    corner: char,
    hborder: &str,
    vborders: usize,
    show_left: bool,
    show_right: bool,
) {
    if show_left {
        buf.push(corner);
    }
    buf.push_str(&hborder.repeat(len - vborders));
    if show_right {
        buf.push(corner);
    }
    buf.push('\n')
}

fn push_line_with_vborder_full(
    buf: &mut String,
    line: &str,
    vborder: char,
    show_left: bool,
    show_right: bool,
) {
    let prev_len = buf.len();
    if show_left {
        buf.push(vborder);
    }
    buf.push_str(line);
    if show_right {
        buf.push(vborder);
    }
    buf.push('\n');
}

fn push_line_with_vborder_padding(
    buf: &mut String,
    line: &str,
    lpad: usize,
    rpad: usize,
    vborder: char,
    show_left: bool,
    show_right: bool,
) {
    push_line_with_vborder_full(
        buf,
        format!("{}{}{}", &PADDING.repeat(lpad), line, &PADDING.repeat(rpad)).as_str(),
        vborder,
        show_left,
        show_right,
    )
}

impl<T: Display> PrintBorder for T {
    fn to_string_border_full_with_struct(
        &self,
        max_width: u16,
        max_height: u16,
        config: Option<&BufferBorder>,
    ) -> String {
        match config {
            None => self.to_string(),
            Some(BufferBorder {
                corner,
                hborder,
                vborder,
                lpad,
                rpad,
                tpad,
                dpad,
                ..
            }) => {
                let corner = *corner;
                let hborder = *hborder;
                let vborder = *vborder;
                let (show_left, show_top, show_bottom, show_right) =
                    config.unwrap().get_borders_shown();
                let hborders = if show_top { 1 } else { 0 } + if show_bottom { 1 } else { 0 };
                let vborders = if show_left { 1 } else { 0 } + if show_right { 1 } else { 0 };
                let max_height = max_height as usize;
                let max_width = max_width as usize;
                let width_without_border = max_width - vborders;
                let lpad = *lpad as usize;
                let rpad = *rpad as usize;
                let tpad = *tpad as usize;
                let dpad = *dpad as usize;
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
                let content = self.to_string();
                let mut ret = String::with_capacity(max_height * (max_width + 1)); // include newline
                if show_top {
                    push_hborder_full(
                        &mut ret, max_width, corner, hborder, vborders, show_left, show_right,
                    );
                }

                for _ in 0..tpad {
                    push_line_with_vborder_full(
                        &mut ret,
                        &PADDING.repeat(width_without_border),
                        vborder,
                        show_left,
                        show_right,
                    );
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
                            show_left,
                            show_right,
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
                            show_left,
                            show_right,
                        );
                        i += end_slice;
                    }
                }

                for _ in 0..dpad {
                    push_line_with_vborder_full(
                        &mut ret,
                        &PADDING.repeat(width_without_border),
                        vborder,
                        show_left,
                        show_right,
                    );
                }
                let rest_space = max_height as isize
                    - (tpad + dpad + content.lines().count() + hborders) as isize;
                for _ in 0..rest_space {
                    push_line_with_vborder_full(
                        &mut ret,
                        &PADDING.repeat(width_without_border),
                        vborder,
                        show_left,
                        show_right,
                    );
                }
                if show_bottom {
                    push_hborder_full(
                        &mut ret, max_width, corner, hborder, vborders, show_left, show_right,
                    );
                }
                ret
            }
        }
    }
    // fn to_string_border_pad(&self, pad: u16) -> String {
    //     self.to_string_border_ipad(CORNER, HBORDER, VBORDER, pad, pad, pad, pad)
    // }
    fn get_auto_size(&self, lpad: u16, rpad: u16, tpad: u16, dpad: u16) -> (u16, u16) {
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
        (max_width, max_height)
    }
    fn to_string_border_ipad(
        &self,
        show_borders: (bool, bool, bool, bool),
        corner: char,
        hborder: &'static str,
        vborder: char,
        lpad: u16,
        rpad: u16,
        tpad: u16,
        dpad: u16,
    ) -> String {
        let (max_width, max_height) = self.get_auto_size(lpad, rpad, tpad, dpad);
        self.to_string_border_full(
            max_height,
            max_width,
            show_borders,
            corner,
            hborder,
            vborder,
            lpad,
            rpad,
            tpad,
            dpad,
        )
    }

    fn to_string_border_full(
        &self,
        max_height: u16,
        max_width: u16,
        show_borders: (bool, bool, bool, bool),
        corner: char,
        hborder: &'static str,
        vborder: char,
        lpad: u16,
        rpad: u16,
        tpad: u16,
        dpad: u16,
    ) -> String {
        let mut border_shown = 0x0;
        if show_borders.0 {
            border_shown |= 1;
        }
        if show_borders.1 {
            border_shown |= 2;
        }
        if show_borders.2 {
            border_shown |= 4;
        }
        if show_borders.3 {
            border_shown |= 8;
        }
        let config = Some(BufferBorder::new(
            border_shown,
            corner,
            hborder,
            vborder,
            lpad,
            rpad,
            tpad,
            dpad,
        ));

        self.to_string_border_full_with_struct(max_width, max_height, config.as_ref())
    }
    fn to_string_border(&self, config: Option<&BufferBorder>) -> String {
        match config {
            None => self.to_string(),
            Some(BufferBorder {
                lpad,
                rpad,
                tpad,
                dpad,
                ..
            }) => {
                let (max_width, max_height) = self.get_auto_size(*lpad, *rpad, *tpad, *dpad);
                self.to_string_border_full_with_struct(max_width, max_height, config)
            }
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
            "H".to_string_border(Some(BufferBorder::default()).as_ref())
        );
    }
}
