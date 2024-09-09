use crate::core::editor::{Buffer, CursorPosition};

#[derive(PartialEq)]
pub enum MotionDirection {
    Foward,
    Backward,
}

pub trait Motion {
    fn get_new_cursor_position(
        &self,
        content: &[String],
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
        content: &[String],
        cursor_position: &CursorPosition,
        direction: MotionDirection,
    ) -> CursorPosition {
        let line_len = content.get(cursor_position.y as usize).unwrap().len();
        if (direction == MotionDirection::Foward && cursor_position.x == line_len as u32 - 1)
            || (direction == MotionDirection::Backward && cursor_position.x == 0)
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

impl Motion for UpDownMotion {
    // BUG: This does not correctly calculate the cursor position with wrapped lines
    fn get_new_cursor_position(
        &self,
        content: &[String],
        cursor_position: &CursorPosition,
        direction: MotionDirection,
    ) -> CursorPosition {
        if (direction == MotionDirection::Foward && cursor_position.y == content.len() as u32 - 1)
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
        let new_line_len: u32 = content.get(new_y).unwrap().len() as u32;
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
        content: &[String],
        cursor_position: &CursorPosition,
        direction: MotionDirection,
    ) -> CursorPosition {
        CursorPosition {
            x: get_char_search(self.0, content, cursor_position, direction, true),
            y: cursor_position.y,
        }
    }
}

impl Motion for UntilWithoutMotion {
    fn get_new_cursor_position(
        &self,
        content: &[String],
        cursor_position: &CursorPosition,
        direction: MotionDirection,
    ) -> CursorPosition {
        CursorPosition {
            x: get_char_search(self.0, content, cursor_position, direction, false),
            y: cursor_position.y,
        }
    }
}

mod test {
    use super::*;

    fn get_content() -> Vec<String> {
        vec![
            "This is a line1".to_string(),
            "This is also a line".to_string(),
            "Another line".to_string(),
            "Guess what another line".to_string(),
        ]
    }

    #[cfg(test)]
    mod left_right {
        use super::*;

        #[test]
        fn normal_foward() {
            let content = get_content();
            let motion = LeftRightMotion;
            let cursor_position = CursorPosition { x: 4, y: 2 };
            assert_eq!(
                motion.get_new_cursor_position(&content, &cursor_position, MotionDirection::Foward),
                CursorPosition { x: 5, y: 2 }
            );
        }

        #[test]
        fn normal_backward() {
            let content = get_content();
            let motion = LeftRightMotion;
            let cursor_position = CursorPosition { x: 4, y: 2 };
            assert_eq!(
                motion.get_new_cursor_position(
                    &content,
                    &cursor_position,
                    MotionDirection::Backward
                ),
                CursorPosition { x: 3, y: 2 }
            );
        }

        #[test]
        fn left_end() {
            let content = get_content();
            let motion = LeftRightMotion;
            let cursor_position = CursorPosition { x: 0, y: 2 };
            assert_eq!(
                motion.get_new_cursor_position(
                    &content,
                    &cursor_position,
                    MotionDirection::Backward
                ),
                CursorPosition { x: 0, y: 2 }
            );
        }

        #[test]
        fn right_end() {
            let content = get_content();
            let motion = LeftRightMotion;
            let line_len = content.get(2).unwrap().len() as u32;
            let cursor_position = CursorPosition {
                x: line_len - 1,
                y: 2,
            };
            assert_eq!(
                motion.get_new_cursor_position(&content, &cursor_position, MotionDirection::Foward),
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

        #[test]
        fn with_foward_normal() {
            let content = get_content();
            let motion = UntilWithMotion('l');
            let cursor_position = CursorPosition { x: 3, y: 0 };
            assert_eq!(
                motion.get_new_cursor_position(&content, &cursor_position, MotionDirection::Foward),
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
                    &content,
                    &cursor_position,
                    MotionDirection::Backward
                ),
                CursorPosition { x: 5, y: 0 }
            )
        }
    }
}
