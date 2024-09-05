pub mod motions;

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct CursorPosition {
    pub x: u32,
    pub y: u32,
}

#[derive(Clone)]
pub struct Buffer {
    pub content: Vec<String>,
    pub cursor_position: CursorPosition,
}

impl Buffer {
    pub async fn new(content: Vec<String>, cursor_position: CursorPosition) -> Buffer {
        Buffer {
            content,
            cursor_position,
        }
    }
}
