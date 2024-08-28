mod motions;

#[derive(Debug, PartialEq)]
pub struct CursorPosition {
    x: u32,
    y: u32,
}

pub struct Buffer {
    content: Vec<String>,
    cursor_position: CursorPosition,
}

impl Buffer {
    async fn new(content: Vec<String>, cursor_position: CursorPosition) -> Buffer {
        Buffer {
            content,
            cursor_position,
        }
    }
}
