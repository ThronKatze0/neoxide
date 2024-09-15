use crate::core::{
    editor::CursorPosition,
    render::manager::{BufferDims, ContentRef},
};

#[derive(PartialEq)]
pub enum MotionDirection {
    Foward,
    Backward,
}

pub trait Motion {
    fn get_new_cursor_position(
        &self,
        buf: impl BufferDims + ContentRef,
        cursor_position: &CursorPosition,
        direction: MotionDirection,
    ) -> CursorPosition;
}

pub struct LeftRightMotion; // h e.g.
pub struct UpDownMotion; // k e.g.
pub struct BeginningWordMotion; // w e.g.
pub struct EndWordMotion; // e e.g.
pub struct UntilWithMotion(char); // f e.g.
pub struct UntilWithoutMotion(char); // t e.g.

impl Motion for LeftRightMotion {
    fn get_new_cursor_position(
        &self,
        buf: impl BufferDims + ContentRef,
        cursor_position: &CursorPosition,
        direction: MotionDirection,
    ) -> CursorPosition {
        let text_len = buf.get_text_len() as u32;
        let start_of_text: u32 = 0;
        let line_len = std::cmp::min(get_line_len(buf, cursor_position.y as usize), text_len);
        if (direction == MotionDirection::Foward && cursor_position.x == line_len as u32 - 1)
            || (direction == MotionDirection::Backward && cursor_position.x == start_of_text.into())
        {
            return CursorPosition {
                x: cursor_position.x,
                y: cursor_position.y,
            };
        }
        CursorPosition {
            x: (cursor_position.x as i32
                + match direction {
                    MotionDirection::Foward => 1,
                    MotionDirection::Backward => -1,
                }) as u32,
            y: cursor_position.y,
        }
    }
}

fn get_lines<T>(buf: &T) -> u32
where
    T: BufferDims + ContentRef,
{
    let mut ret = 0;
    for line in buf.content().iter() {
        ret += line.len() as u32 / buf.get_text_len() as u32 + 1;
    }
    ret
}
fn get_line_len(buf: impl BufferDims + ContentRef, y: usize) -> u32 {
    let mut line_idx = 0;
    let mut vec_idx = 0;
    let mut start_idx = 0;
    let content = buf.content();
    let text_len = buf.get_text_len() as usize - 2; // no idea why -2 is there
    while line_idx < y {
        if content[vec_idx].len() - start_idx <= text_len {
            start_idx = 0;
            vec_idx += 1;
        } else {
            start_idx += text_len;
        }
        line_idx += 1;
    }
    (content[vec_idx].len() - start_idx) as u32
}

impl Motion for UpDownMotion {
    fn get_new_cursor_position(
        &self,
        buf: impl BufferDims + ContentRef,
        cursor_position: &CursorPosition,
        direction: MotionDirection,
    ) -> CursorPosition {
        let len = get_lines(&buf);
        if (direction == MotionDirection::Foward
            && cursor_position.y == std::cmp::min(buf.height(), len as u16) as u32 - 1)
            || (direction == MotionDirection::Backward && cursor_position.y == 0)
        {
            return CursorPosition {
                x: cursor_position.x,
                y: cursor_position.y,
            };
        }
        let new_y = (cursor_position.y as i32
            + match direction {
                MotionDirection::Foward => 1,
                MotionDirection::Backward => -1,
            }) as usize;
        let new_line_len: u32 = get_line_len(buf, new_y); //buf.content().get(new_y).unwrap().len() as u32;
        if new_line_len - 1 <= cursor_position.x {
            CursorPosition {
                x: new_line_len - 1,
                y: new_y as u32,
            }
        } else {
            CursorPosition {
                x: cursor_position.x,
                y: new_y as u32,
            }
        }
    }
}

fn get_char_search(
    chr: char,
    content: &[String],
    cursor_position: &CursorPosition,
    direction: MotionDirection,
    with_search_result: bool,
) -> u32 {
    let line = content.get(cursor_position.y as usize).unwrap();
    let search_area = match direction {
        MotionDirection::Foward => &line[cursor_position.x as usize..],
        MotionDirection::Backward => &line[..cursor_position.x as usize],
    };
    // currently writing this code in a car in turkmenistan :(
    let callback = |curr_chr| curr_chr == chr;
    let mut it = search_area.chars().into_iter();
    let search_result = match direction {
        MotionDirection::Foward => it.position(callback),
        MotionDirection::Backward => it.rev().position(callback),
    };
    if let Some(position) = search_result {
        let position = match direction {
            MotionDirection::Foward => position + cursor_position.x as usize,
            MotionDirection::Backward => cursor_position.x as usize - position - 1,
        };
        if with_search_result {
            return position as u32;
        } else {
            return match direction {
                MotionDirection::Foward => position as u32 - 1,
                MotionDirection::Backward => position as u32 + 1,
            };
        }
    }
    cursor_position.x
}

impl Motion for UntilWithMotion {
    fn get_new_cursor_position(
        &self,
        buf: impl BufferDims + ContentRef,
        cursor_position: &CursorPosition,
        direction: MotionDirection,
    ) -> CursorPosition {
        CursorPosition {
            x: get_char_search(self.0, buf.content(), cursor_position, direction, true),
            y: cursor_position.y,
        }
    }
}

impl Motion for UntilWithoutMotion {
    fn get_new_cursor_position(
        &self,
        buf: impl BufferDims + ContentRef,
        cursor_position: &CursorPosition,
        direction: MotionDirection,
    ) -> CursorPosition {
        CursorPosition {
            x: get_char_search(self.0, buf.content(), cursor_position, direction, false),
            y: cursor_position.y,
        }
    }
}

mod test {
    use crate::core::render::manager::BufferBorder;

    use super::*;

    struct TestBuffer {
        width: u16,
        height: u16,
        offx: u16,
        offy: u16,
        border: BufferBorder,
        content: Vec<String>,
    }
    impl ContentRef for TestBuffer {
        fn content(&self) -> &Vec<String> {
            &self.content
        }
    }
    impl BufferDims for TestBuffer {
        fn lpad(&self) -> u16 {
            self.border.lpad
        }
        fn height(&self) -> u16 {
            self.height
        }
        fn rpad(&self) -> u16 {
            self.border.rpad
        }
        fn tpad(&self) -> u16 {
            self.border.tpad
        }
        fn dpad(&self) -> u16 {
            self.border.dpad
        }
        fn width(&self) -> u16 {
            self.width
        }
        fn offy(&self) -> u16 {
            self.offy
        }
        fn offx(&self) -> u16 {
            self.offx
        }
        fn get_text_len(&self) -> u16 {
            self.width - self.lpad() - self.rpad()
        }
    }

    fn get_content() -> TestBuffer {
        TestBuffer {
            width: 20,
            height: 4,
            offx: 0,
            offy: 0,
            border: BufferBorder::blank(),
            content: vec![
                "This is a line1".to_string(),
                "This is also a line".to_string(),
                "Another line".to_string(),
                "Guess what another line".to_string(),
            ],
        }
    }

    #[cfg(test)]
    mod left_right {
        use futures::executor::block_on;

        use super::*;

        #[test]
        fn normal_foward() {
            let content = get_content();
            let motion = LeftRightMotion;
            let cursor_position = CursorPosition { x: 4, y: 2 };
            assert_eq!(
                block_on(motion.get_new_cursor_position(
                    content,
                    &cursor_position,
                    MotionDirection::Foward
                )),
                CursorPosition { x: 5, y: 2 }
            );
        }

        #[test]
        fn normal_backward() {
            let content = get_content();
            let motion = LeftRightMotion;
            let cursor_position = CursorPosition { x: 4, y: 2 };
            assert_eq!(
                block_on(motion.get_new_cursor_position(
                    content,
                    &cursor_position,
                    MotionDirection::Backward
                )),
                CursorPosition { x: 3, y: 2 }
            );
        }

        #[test]
        fn left_end() {
            let content = get_content();
            let motion = LeftRightMotion;
            let cursor_position = CursorPosition { x: 0, y: 2 };
            assert_eq!(
                block_on(motion.get_new_cursor_position(
                    content,
                    &cursor_position,
                    MotionDirection::Backward
                )),
                CursorPosition { x: 0, y: 2 }
            );
        }

        #[test]
        fn right_end() {
            let content = get_content();
            let motion = LeftRightMotion;
            let line_len = content.content().get(2).unwrap().len() as u32;
            let cursor_position = CursorPosition {
                x: line_len - 1,
                y: 2,
            };
            assert_eq!(
                motion.get_new_cursor_position(content, &cursor_position, MotionDirection::Foward),
                CursorPosition {
                    x: line_len - 1,
                    y: 2
                }
            );
        }
    }

    #[cfg(test)]
    mod until {
        use super::*;
        use futures::executor::block_on;

        #[test]
        fn with_foward_normal() {
            let content = get_content();
            let motion = UntilWithMotion('l');
            let cursor_position = CursorPosition { x: 3, y: 0 };
            assert_eq!(
                motion.get_new_cursor_position(content, &cursor_position, MotionDirection::Foward),
                CursorPosition { x: 10, y: 0 }
            )
        }

        #[test]
        fn with_backward_normal() {
            let content = get_content();
            let motion = UntilWithMotion('i');
            let cursor_position = CursorPosition { x: 10, y: 0 };
            assert_eq!(
                motion.get_new_cursor_position(
                    content,
                    &cursor_position,
                    MotionDirection::Backward
                ),
                CursorPosition { x: 5, y: 0 }
            )
        }
    }
}
