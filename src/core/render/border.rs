use super::manager::{Buffer, BufferBorder, RenderBuffer};
use std::cmp::min;
use std::fmt::Display;
// TODO: change to unicode
pub const HBORDER: &str = "─";
pub const VBORDER: char = '│';
pub const _DOUBLE_LINEAR_BORDER: char = '=';
pub const CORNER: char = '+';
pub const PADDING: &str = " ";
mod param_structs;
use param_structs::{CreateLineParams, WriteLineParams};

#[inline]
async fn write_str(render_buf: &mut RenderBuffer, params: &mut WriteLineParams<'_>) {
    // let end = min(
    //     params.width_without_border as usize - (params.border.rpad + params.border.lpad) as usize,
    //     params.line.len(),
    // );
    render_buf
        .write_str(
            params.offx,
            params.offy,
            params.term_width,
            params.line, // may need to also consider padding
        )
        .await;
    params.offx += params.line.len();
}

#[inline]
async fn write_corner(
    render_buf: &mut RenderBuffer,
    params: &mut WriteLineParams<'_>,
    border_pos: usize,
) {
    if params.borders_shown[border_pos] {
        render_buf
            .write(
                params.offx,
                params.offy,
                params.term_width,
                params.border.corner[border_pos],
            )
            .await;
        params.offx += 1;
    }
}

#[inline]
async fn write_border(
    render_buf: &mut RenderBuffer,
    params: &mut WriteLineParams<'_>,
    border_pos: usize,
) {
    if params.borders_shown[border_pos] {
        render_buf
            .write(
                params.offx,
                params.offy,
                params.term_width,
                params.border.vborder,
            )
            .await;
        params.offx += 1;
    }
}

#[inline]
async fn write_line_without_padding(
    render_buf: &mut RenderBuffer,
    params: &mut WriteLineParams<'_>,
) {
    params.offx = params.orig_offx;
    write_str(render_buf, params).await;
    params.offy += 1;
}

#[inline]
async fn write_padding<'a>(
    render_buf: &mut RenderBuffer,
    params: &mut WriteLineParams<'_>,
    pad: u16,
) {
    let blank = PADDING.repeat(pad as usize);
    let mut temp_padding = WriteLineParams {
        line: &blank,
        ..*params
    };
    write_str(render_buf, &mut temp_padding).await;
    params.offx = temp_padding.offx;
}

#[inline]
async fn write_line_with_padding(render_buf: &mut RenderBuffer, params: &mut WriteLineParams<'_>) {
    params.offx = params.orig_offx;
    write_border(render_buf, params, 0).await;

    write_padding(render_buf, params, params.border.lpad).await;

    write_str(render_buf, params).await;

    write_padding(render_buf, params, params.border.rpad).await;
    let rest_space = params.width_without_border
        - (params.offx - params.orig_offx - if params.borders_shown[0] { 1 } else { 0 }) as u16;
    write_padding(render_buf, params, rest_space).await;

    write_border(render_buf, params, 3).await;
    params.offy += 1;
}

#[inline]
fn create_line(params: &CreateLineParams) -> String {
    let mut ret = String::with_capacity(params.width as usize);
    if params.show_left {
        ret.push(params.cornerl);
    }
    ret.push_str(&params.filler.repeat(params.width_without_border as usize));
    if params.show_right {
        ret.push(params.cornerr);
    }
    ret
}

impl Buffer {
    pub async fn render(&self, term_width: u16, render_buf: &mut RenderBuffer) {
        let content = self.to_string();
        let border = self.border();
        let offsets = self.offsets();
        let (offx, offy) = (offsets.0 as usize, offsets.1 as usize);
        match border {
            Some(border) => {
                let borders_shown = border.get_borders_shown();
                let (width, height) = self.size();
                let (vborders, hborders) = border.get_number_of_borders();
                let width_without_border = width - vborders;
                let height_without_border = height - hborders;
                let cl_params = CreateLineParams {
                    width,
                    show_left: borders_shown[0],
                    show_right: borders_shown[3],
                    cornerl: border.corner[0],
                    cornerr: border.corner[1],
                    filler: border.hborder,
                    width_without_border,
                };
                let hborder = create_line(&cl_params);
                // {
                //     let mut ret = String::with_capacity(width as usize);
                //     if borders_shown[0] {
                //         ret.push(border.corner[0]);
                //     }
                //     ret.push_str(&border.hborder.repeat(width_without_border as usize));
                //     if borders_shown[3] {
                //         ret.push(border.corner[1]);
                //     }
                //     ret
                // };
                let blank = create_line(&CreateLineParams {
                    cornerl: border.vborder,
                    cornerr: border.vborder,
                    filler: PADDING,
                    ..cl_params
                });
                // {
                //     let mut ret = String::with_capacity(width as usize);
                //     ret.push(border.vborder);
                //     ret.push_str(&PADDING.repeat(width_without_border as usize));
                //     ret.push(border.vborder);
                //     ret
                // };

                let mut params = WriteLineParams {
                    offx,
                    orig_offx: offx,
                    offy,
                    term_width,
                    width_without_border,
                    line: &hborder,
                    border,
                    borders_shown,
                };
                if borders_shown[1] {
                    write_line_without_padding(render_buf, &mut params).await;
                }

                let orig_offy = params.offy;
                params.line = &blank;
                for _ in 0..border.tpad {
                    write_line_without_padding(render_buf, &mut params).await;
                }

                let mut i = 0;
                let mut skip_newline = false;
                while i < content.len() {
                    let end_idx = if let Some(newline_idx) = content[i..].find('\n') {
                        skip_newline = true;
                        i + newline_idx
                    } else {
                        min(i + width_without_border as usize, content.len())
                    };
                    params.line = &content[i..end_idx];
                    write_line_with_padding(render_buf, &mut params).await;
                    i += end_idx;
                    if skip_newline {
                        skip_newline = false;
                        i += 1;
                    }
                }

                params.line = &blank;
                for _ in 0..border.dpad {
                    write_line_without_padding(render_buf, &mut params).await;
                }

                let rest_space: i16 =
                    height_without_border as i16 - (params.offy - orig_offy) as i16;
                for _ in 0..rest_space {
                    write_line_without_padding(render_buf, &mut params).await;
                }

                // assert_eq!(height_without_border - (params.offy - orig_offy) as u16, 0);

                let hborder = create_line(&CreateLineParams {
                    cornerl: border.corner[3],
                    cornerr: border.corner[2],
                    ..cl_params
                });
                // {
                //     let mut ret = String::with_capacity(width as usize);
                //     if borders_shown[0] {
                //         ret.push(border.corner[3]);
                //     }
                //     ret.push_str(&border.hborder.repeat(width_without_border as usize));
                //     if borders_shown[3] {
                //         ret.push(border.corner[2]);
                //     }
                //     ret
                // };
                // params.line = "";
                // hborder.replace_range(0..1, &border.corner[3].to_string());
                params.line = &hborder;
                if borders_shown[2] {
                    write_line_without_padding(render_buf, &mut params).await;
                }
            }
            None => {
                render_buf
                    .write_str(offx as usize, offy as usize, term_width, &content)
                    .await
            }
        }
    }
}

pub trait PrintBorder: Display {
    // fn to_string_border_pad(&self, pad: u16) -> String;
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
    // async fn render(&self, render_buf: &mut RenderBuffer) {
    //     todo!()
    // }
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
                let arr = config.unwrap().get_borders_shown();
                let (show_left, show_top, show_bottom, show_right) =
                    (arr[0], arr[1], arr[2], arr[3]);
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
                assert!(lpad + rpad <= max_width, "{lpad} (lpad) + {rpad} (rpad) is greater than specified max_width ({max_width})");
                assert!(tpad + dpad <= max_height, "{tpad} (tpad) + {dpad} (dpad) is greater than specified max_height ({max_height})");
                assert!(max_width > 1 && max_height > 1);
                let content = self.to_string();
                let mut ret = String::with_capacity(max_height * (max_width + 1)); // include newline
                if show_top {
                    push_hborder_full(
                        &mut ret,
                        max_width,
                        (corner[0], corner[1]),
                        hborder,
                        vborders,
                        show_left,
                        show_right,
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
                        &mut ret,
                        max_width,
                        (corner[2], corner[3]),
                        hborder,
                        vborders,
                        show_left,
                        show_right,
                    );
                }
                ret
            }
        }
    }
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

    #[deprecated(
        since = "0.1.0",
        note = "please use to_string_border_full_with_struct instead"
    )]
    fn to_string_border_full(
        &self,
        max_height: u16,
        max_width: u16,
        borders_shown: (bool, bool, bool, bool),
        corner: char,
        hborder: &'static str,
        vborder: char,
        lpad: u16,
        rpad: u16,
        tpad: u16,
        dpad: u16,
    ) -> String {
        let mut border_shown = 0x0;
        // this is the reason why you don't use tuples, but this is deprecated anyways so who cares
        // how awful this code is
        if borders_shown.0 {
            border_shown |= 1;
        }
        if borders_shown.1 {
            border_shown |= 2;
        }
        if borders_shown.2 {
            border_shown |= 4;
        }
        if borders_shown.3 {
            border_shown |= 8;
        }
        let config = Some(BufferBorder::new(
            border_shown,
            [corner; 4],
            hborder,
            vborder,
            lpad,
            rpad,
            tpad,
            dpad,
        ));

        self.to_string_border_full_with_struct(max_width, max_height, config.as_ref())
    }
}

fn push_hborder_full(
    buf: &mut String,
    len: usize,
    corner: (char, char),
    hborder: &str,
    vborders: usize,
    show_left: bool,
    show_right: bool,
) {
    if show_left {
        buf.push(corner.0);
    }
    buf.push_str(&hborder.repeat(len - vborders));
    if show_right {
        buf.push(corner.1);
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
    // fn to_string_border_pad(&self, pad: u16) -> String {
    //     self.to_string_border_ipad(CORNER, HBORDER, VBORDER, pad, pad, pad, pad)
    // }

    fn to_string_border_full(
        &self,
        max_height: u16,
        max_width: u16,
        borders_shown: (bool, bool, bool, bool),
        corner: char,
        hborder: &'static str,
        vborder: char,
        lpad: u16,
        rpad: u16,
        tpad: u16,
        dpad: u16,
    ) -> String {
        let mut border_shown = 0x0;
        // this is the reason why you don't use tuples, but this is deprecated anyways so who cares
        // how awful this code is
        if borders_shown.0 {
            border_shown |= 1;
        }
        if borders_shown.1 {
            border_shown |= 2;
        }
        if borders_shown.2 {
            border_shown |= 4;
        }
        if borders_shown.3 {
            border_shown |= 8;
        }
        let config = Some(BufferBorder::new(
            border_shown,
            [corner; 4],
            hborder,
            vborder,
            lpad,
            rpad,
            tpad,
            dpad,
        ));

        self.to_string_border_full_with_struct(max_width, max_height, config.as_ref())
    }
}

// #[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_world_no_padding() {
        let config = BufferBorder::blank();
        assert_eq!(
            "+-----+\n|Hello|\n+-----+\n",
            "Hello".to_string_border_full_with_struct(7, 3, Some(&config)),
        )
    }

    #[test]
    fn hello_world_no_padding_mean_newlines() {
        let config = BufferBorder::blank();
        assert_eq!(
            "+-----+\n|Hello|\n|World|\n+-----+\n",
            "Hello\nWorld".to_string_border_full_with_struct(7, 4, Some(&config))
        )
    }

    #[test]
    fn hpaddings_eq() {
        let mut config = BufferBorder::blank();
        config.lpad = 1;
        config.rpad = 1;
        assert_eq!(
            "+---+\n| H |\n+---+\n",
            "H".to_string_border_full_with_struct(5, 3, Some(&config))
        );
    }

    #[test]
    fn hpaddings_neq() {
        let mut config = BufferBorder::blank();
        config.lpad = 1;
        assert_eq!(
            "+--+\n| H|\n+--+\n",
            "H".to_string_border_full_with_struct(4, 3, Some(&config))
        );
    }

    #[test]
    fn vpaddings_eq() {
        let mut config = BufferBorder::blank();
        config.tpad = 1;
        config.dpad = 1;
        assert_eq!(
            "+-+\n| |\n|H|\n| |\n+-+\n",
            "H".to_string_border_full_with_struct(3, 3, Some(&config))
        );
    }

    #[test]
    fn vpaddings_neq() {
        let mut config = BufferBorder::blank();
        config.tpad = 1;
        assert_eq!(
            "+-+\n| |\n|H|\n+-+\n",
            "H".to_string_border_full_with_struct(3, 3, Some(&config))
        );
    }

    #[test]
    fn overflow() {
        // h: 3 w: 7
        let config = BufferBorder::blank();
        assert_eq!(
            "+-----+\n|Hello|\n|World|\n+-----+\n",
            "HelloWorld".to_string_border_full_with_struct(7, 3, Some(&config))
        )
    }

    #[test]
    fn width_detection() {
        assert_eq!(
            "+-----+\n|Hello|\n|foo  |\n+-----+\n",
            "Hello\nfoo".to_string_border_ipad(
                (true, true, true, true),
                CORNER,
                HBORDER,
                VBORDER,
                0,
                0,
                0,
                0
            )
        );
    }

    #[test]
    fn width_detection_padding() {
        assert_eq!(
            "+------+\n| Hello|\n| foo  |\n+------+\n",
            "Hello\nfoo".to_string_border_ipad(
                (true, true, true, true),
                CORNER,
                HBORDER,
                VBORDER,
                1,
                0,
                0,
                0
            )
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
