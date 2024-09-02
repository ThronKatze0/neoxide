use std::io::{stdout, Write};

use crossterm::{cursor::MoveTo, queue, style::Print};

// static BUFFER_MAN_SINGLETON: Lazy<BufferManager> = Lazy::new(|| {
//     BufferManager::new();
// })

mod border;
pub mod manager;

pub use manager::ClientBuffer;

fn print_text(text: &str, x: u16, y: u16) -> std::io::Result<()> {
    queue!(stdout(), MoveTo(x, y), Print(text))?;
    Ok(())
}

fn print_lines(text: &str, start_row: u16, padding_left: u16) -> std::io::Result<()> {
    text.lines()
        .enumerate()
        .take_while(|(i, line)| print_text(line, padding_left, start_row + *i as u16).is_ok())
        .for_each(drop);
    stdout().flush()?;
    Ok(())
}

// trait BufferManagerChildren: PrintBorder + Send + Renderable {}
// const CHARS_PER_EL: usize = 32;
// #[derive(Debug)]
// struct BufferManager {
//     free_space: Arc<Mutex<Vec<Vec<u32>>>>,
//     render_buffer: Arc<Mutex<Vec<u32>>>,
//     term_width: u16,
//     term_height: u16,
// }
//
// impl BufferManager {
//     pub fn new() -> Self {
//         let (term_width, term_height) = terminal::size().expect(
//             "Buffer Manager cannot be created, because the terminal size cannot be queried",
//         );
//         BufferManager::new_custom(term_width, term_height)
//     }
//
//     /// returns the current highest layer the buffer knows
//     /// to ensure memory compactness, all buffers are advised to first consult with this before
//     /// setting their layer
//     pub async fn get_cur_highest_layer(&self) -> usize {
//         self.free_space.lock().await.len() - 1
//     }
//
//     // NOTE: I think if I make this take ``&self``, then there would a mutable reference and a
//     // immutable reference to free_space at the same time
//     fn add_layer(term_width: u16, term_height: u16, layers: &mut Vec<Vec<u32>>) {
//         let mut new_layer =
//             Vec::with_capacity((term_width as usize * term_height as usize) / CHARS_PER_EL + 1);
//         for _ in 0..new_layer.capacity() {
//             new_layer.push(0);
//         }
//         layers.push(new_layer);
//     }
//
//     fn new_custom(term_width: u16, term_height: u16) -> Self {
//         let mut free_space = Vec::with_capacity(2);
//         BufferManager::add_layer(term_width, term_height, &mut free_space);
//         BufferManager {
//             free_space: Arc::new(Mutex::new(free_space)),
//             render_buffer: Arc::new(Mutex::new(Vec::with_capacity(
//                 (term_width as usize * term_height as usize) / CHARS_PER_EL + 1,
//             ))),
//             term_width,
//             term_height,
//         }
//     }
//
//     fn validate_bitarea(free_space: &mut Vec<u32>, start: u16, end: u16) -> bool {
//         let start = start as usize;
//         let end = end as usize;
//
//         let mut start_idx = start / CHARS_PER_EL;
//         let end_idx = end / CHARS_PER_EL;
//         let end_bits_off = end - end_idx * CHARS_PER_EL;
//         let mut start_bit_idx = start;
//         let mut masks: Vec<u32> = Vec::with_capacity(end / CHARS_PER_EL - start_idx + 1);
//         while start_idx <= end_idx {
//             let bits_off = start_bit_idx - start_idx * CHARS_PER_EL;
//             let bits = free_space[start_idx];
//             // dbg!(start_bit_idx);
//             // dbg!(bits_off);
//             // compare_mask will represent the space the new buffer will need in the current u32
//             let compare_mask = if start_idx == end_idx {
//                 let mut res = u32::MAX;
//                 res ^= (1 << bits_off) - 1; // zeroes everything before start
//                                             // println!("{:b}", res);
//                 res ^= u32::MAX ^ ((1 << end_bits_off) - 1); // zeroes everything after end
//                                                              // println!("{:b}", res);
//                 res
//             } else {
//                 u32::MAX ^ (1 << bits_off) - 1 // TODO: this can be refactored into the upper
//                                                // block
//             };
//             // println!("bits: {:b}", bits);
//             // println!("compmask: {:b}", compare_mask);
//             // println!("{:b}", bits ^ compare_mask);
//             // correspond to the chars, the buffer wants to write to (in the current u32)
//             if bits & compare_mask > 0 {
//                 println!("Could not fit new buffer part in");
//                 return false;
//             }
//             masks.push(compare_mask);
//             start_bit_idx = (start_idx + 1) * CHARS_PER_EL;
//             start_idx = start_bit_idx / CHARS_PER_EL;
//         }
//         // dbg!(&masks);
//         masks
//             .into_iter()
//             .enumerate()
//             .for_each(|(i, mask)| free_space[start / CHARS_PER_EL + i] ^= mask);
//         true
//     }
//
//     /// checks if the buffer can be rendered/updates the bitmap if it can better
//     /// this will probably not be used
//     async fn check_space(&mut self, buf: &Buffer<'_, impl Display>) -> bool {
//         if buf.offx + buf.width >= self.term_width || buf.offy + buf.height >= self.term_height {
//             println!("Out of bounce!");
//             return false;
//         }
//
//         let mut lock = self.free_space.lock().await;
//         if lock.len() <= buf.layer.into() {
//             // too much is better than too little i
//             // guess?
//             for _ in lock.len()..=buf.layer.into() {
//                 BufferManager::add_layer(self.term_width, self.term_height, &mut lock);
//             }
//         }
//
//         let box_start = buf.offx + buf.offy * self.term_width;
//         for i in 0..buf.height {
//             let start = box_start + i * self.term_width;
//             let end = start + buf.width;
//             if !BufferManager::validate_bitarea(&mut lock[buf.layer as usize], start, end) {
//                 return false;
//             }
//         }
//         true
//     }
// }
//
// #[cfg(test)]
// mod tests {
//     use super::*;
//     // NOTE: This is the only way to get async to work with tests
//     use futures::executor::block_on;
//
//     #[test]
//     fn test_length_calc() {
//         let bm = BufferManager::new_custom(32, 1);
//         dbg!(&bm);
//         assert_eq!(block_on(bm.free_space.lock())[0].len(), 2);
//     }
//
//     #[test]
//     fn test_single_full() {
//         let bm = BufferManager::new_custom(32, 1);
//         // block_on(bm.free_space.lock())[0] = u32::MAX;
//         assert!(BufferManager::validate_bitarea(
//             &mut block_on(bm.free_space.lock())[0],
//             0,
//             32
//         ));
//         assert_eq!(block_on(bm.free_space.lock())[0][0], u32::MAX);
//     }
//     #[test]
//     fn test_single_partial() {
//         let bm = BufferManager::new_custom(32, 1);
//         block_on(bm.free_space.lock())[0][0] = 0xFF0000FF;
//         assert!(BufferManager::validate_bitarea(
//             &mut block_on(bm.free_space.lock())[0],
//             8,
//             24
//         ));
//         assert_eq!(block_on(bm.free_space.lock())[0][0], u32::MAX);
//     }
//     #[test]
//     fn test_fail() {
//         let bm = BufferManager::new_custom(32, 1);
//         block_on(bm.free_space.lock())[0][0] = u32::MAX; // no space left
//         assert!(!BufferManager::validate_bitarea(
//             &mut block_on(bm.free_space.lock())[0],
//             0,
//             8
//         ));
//     }
//
//     #[test]
//     fn test_overlapping() {
//         let bm = BufferManager::new_custom(32, 1);
//         block_on(bm.free_space.lock())[0][0] = 0x0000FFFF;
//         block_on(bm.free_space.lock())[0][1] = 0xFFFFFF00;
//         assert!(BufferManager::validate_bitarea(
//             &mut block_on(bm.free_space.lock())[0],
//             16,
//             40
//         ));
//         assert_eq!(block_on(bm.free_space.lock())[0][0], u32::MAX);
//         assert_eq!(block_on(bm.free_space.lock())[0][1], u32::MAX);
//     }
//
//     #[test]
//     fn test_segmented() {
//         let bm = BufferManager::new_custom(32, 1);
//         block_on(bm.free_space.lock())[0][0] = 0xFF0000FF;
//         block_on(bm.free_space.lock())[0][1] = 0x000000FF;
//         assert!(BufferManager::validate_bitarea(
//             &mut block_on(bm.free_space.lock())[0],
//             8,
//             24
//         ));
//     }
//
//     #[test]
//     fn test_conv() {
//         let mut bm = BufferManager::new_custom(128, 128);
//
//         let mut buf = Buffer::default("");
//         buf.width = 5;
//         buf.height = 5;
//         buf.children.push("T");
//
//         assert!(block_on(bm.check_space(&buf)));
//     }
// }
