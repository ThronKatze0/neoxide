use super::*;

#[derive(Clone, Debug)]
pub struct WriteLineParams<'a> {
    pub offx: usize,
    pub orig_offx: usize,
    pub offy: usize,
    pub term_width: u16,
    pub width_without_border: u16,
    pub line: &'a str,
    pub border: &'a BufferBorder,
    pub borders_shown: [bool; 4],
}

pub struct CreateLineParams<'a> {
    pub width: u16,
    pub show_left: bool,
    pub show_right: bool,
    pub cornerl: char,
    pub cornerr: char,
    pub filler: &'a str,
    pub width_without_border: u16,
}
