use crate::core::editor::{Buffer as MotionBuffer, CursorPosition};
use crate::core::event_handling::{EventCallback, EventHandler};
use crate::core::logger::{self, LogLevel};

use super::border::{PrintBorder, CORNER, HBORDER, VBORDER};
use async_trait::async_trait;
use crossterm::style::Print;
use crossterm::terminal::{Clear, ClearType};
use crossterm::{queue, terminal, QueueableCommand};
use downcast_rs::{impl_downcast, DowncastSync};
use futures::executor::block_on;
use once_cell::sync::Lazy;
use std::io::{stdout, Write};
use std::ops::Range;
use std::sync::Arc;
use std::{collections::HashMap, fmt::Display};
use strum_macros::EnumCount;
use tokio::sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};

// NOTE: putting one big lock on the entire buffer manager could hurt performance, look into making
// one lock per layer instead
static BUFMAN_GLOB: Lazy<RwLock<BufferManager>> = Lazy::new(|| RwLock::new(BufferManager::new()));
static RENDER_EVH: Lazy<EventHandler<Event, EventData>> = Lazy::new(|| {
    let evh = EventHandler::new();
    let callback: EventCallback<Event, _> = EventCallback::new(
        Arc::new(Box::new(|_: Arc<Mutex<_>>| {
            let fut = async { BUFMAN_GLOB.write().await.resize().await.unwrap() };
            Box::pin(fut)
        })),
        true,
        Event::Resize,
    );
    block_on(evh.subscribe(callback));
    evh
});
async fn bufman_read<'a>() -> RwLockReadGuard<'a, BufferManager> {
    BUFMAN_GLOB.read().await
}
async fn bufman_write<'a>() -> RwLockWriteGuard<'a, BufferManager> {
    BUFMAN_GLOB.write().await
}
unsafe impl Sync for BufferManager {}

#[derive(Clone, Copy, EnumCount)]
enum Event {
    Resize,
}
struct EventData;
unsafe impl Sync for Event {}
unsafe impl Sync for EventData {}
async fn set_resize_events() {}

#[derive(Clone)]
struct BufferRef {
    layer: u8,
    id: BufferId,
}

type BufferId = u32; // NOTE: just don't create 2^32-1 buffers on one layer
pub struct ClientBuffer {
    bufman_ref: BufferRef,
    motion_stuff: MotionBuffer,
}

const CLIENTBUF_ID_ERR: &str =
    "BUG: ClientBuffer ({id}) has an invalid ID! Tried to access on layer {layer}";

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum ColorValue {
    Red,
    Green,
    Blue,
    Custom(u8, u8, u8),
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum ANSICode {
    SetCursor(u16, u16),
    Reset,
    Color(bool, ColorValue),
}

const CSI: &str = "\x1B[";

impl ANSICode {
    fn reset() -> Self {
        ANSICode::Reset
    }
    pub fn color(foreground: bool, (r, g, b): (u8, u8, u8)) -> Self {
        ANSICode::Color(foreground, ColorValue::Custom(r, g, b))
    }
    pub fn fcustom(r: u8, g: u8, b: u8) -> Self {
        Self::color(true, (r, g, b))
    }
    fn conv(&self) -> String {
        let mut ret = String::with_capacity(4);
        ret.push_str(CSI);
        match self {
            ANSICode::Reset => ret.push_str("0m"),
            ANSICode::SetCursor(x, y) => ret.push_str(format!("{};{}H", x + 1, y + 1).as_str()),
            ANSICode::Color(foreground, color) => {
                ret.push(if *foreground { '3' } else { '4' });
                ret.push_str("8;2;");
                match color {
                    ColorValue::Red => ret.push_str("255;0;0"),
                    ColorValue::Green => ret.push_str("0;255;0"),
                    ColorValue::Blue => ret.push_str("0;0;255"),
                    ColorValue::Custom(r, g, b) => ret.push_str(format!("{r};{g};{b}").as_str()),
                    _ => todo!(),
                }
                ret.push('m');
            }
        }
        ret
    }
}
impl Display for ANSICode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.conv())
    }
}

impl ClientBuffer {
    pub async fn focus(&self) -> Result<(), &str> {
        BUFMAN_GLOB
            .write()
            .await
            .change_focus(self.bufman_ref.clone())
            .await?;
        Ok(())
    }
    fn bufman_ref(&self) -> BufferRef {
        self.bufman_ref.clone()
    }
    fn id(&self) -> BufferId {
        self.bufman_ref.id
    }
    fn layer(&self) -> u8 {
        self.bufman_ref.layer
    }

    pub async fn set_color(&mut self, range: Range<usize>, color: ANSICode) {
        let mut handle = BUFMAN_GLOB.write().await;

        let buf = handle.get_buf_mut(self.layer(), self.id()).expect(
            format!(
                "Childless client buffer(layer={}, id={})!",
                self.layer(),
                self.id()
            )
            .as_str(),
        );
        buf.ctrl_codes.push((color, range.start));
        buf.ctrl_codes.push((ANSICode::reset(), range.end));
    }
    pub async fn set_content(&mut self, content: String) -> Result<(), String> {
        self.motion_stuff.content = content.lines().map(|str| str.to_string()).collect();
        let mut handle = BUFMAN_GLOB.write().await;
        let BufferRef { layer, id } = self.bufman_ref;
        let buf = handle.get_buf_mut(layer, id)?;
        buf.content = content;
        logger::log(LogLevel::Normal, "start rerendering").await;
        if let Err(err) = handle.rerender().await {
            return Err(format!("Error when rerendering: {err}"));
        }
        logger::log(LogLevel::Normal, "finish rerendering (for realz)").await;
        Ok(())
    }
    pub async fn build(id: BufferId, tiled: bool) -> Result<Self, String> {
        let mut handle = bufman_write().await;
        let vec = if tiled {
            &handle.tiled_layouts
        } else {
            &handle.free_layouts
        };
        for layer in vec.iter() {
            if !handle.layers[*layer].is_full() {
                let layer = *layer as u8;
                let id = handle.add_new_buf(layer, id).await?;
                return Ok(ClientBuffer {
                    bufman_ref: BufferRef { layer, id },
                    motion_stuff: MotionBuffer::new(Vec::new(), CursorPosition { x: 0, y: 0 })
                        .await,
                });
            }
        }
        Err("all layers full!".to_string())
    }

    pub async fn build_on_tiled(id: BufferId) -> Result<Self, String> {
        ClientBuffer::build(id, true).await
    }
    pub async fn build_on_free(id: BufferId) -> Result<Self, String> {
        ClientBuffer::build(id, false).await
    }

    pub async fn move_to_layer(&mut self, layer: u8) -> Result<(), String> {
        let mut handle = BUFMAN_GLOB.write().await;
        let buf = handle
            .rem_buf(self.bufman_ref.layer.into(), self.bufman_ref.id)
            .await
            .expect(CLIENTBUF_ID_ERR);
        if let Err(msg) = handle.add_buf(layer, self.bufman_ref.id, buf).await {
            return Err(format!("adding buffer failed: {}", msg));
        }
        Ok(())
    }

    pub fn cursor_position(&self) -> &CursorPosition {
        &self.motion_stuff.cursor_position
    }
    pub async fn get_content(&self) -> &Vec<String> {
        &self.motion_stuff.content
    }

    pub async fn center(&self) {
        BUFMAN_GLOB
            .write()
            .await
            .get_buf_mut(self.layer(), self.id())
            .expect("Orphaned Clientbuffer!")
            .center()
    }
}

impl Drop for ClientBuffer {
    fn drop(&mut self) {
        let BufferRef { layer, id } = self.bufman_ref;
        // this is the best i can do for now
        // .blocking_write() crashes the entire program (no wonder, tokio doesn't like it when you
        // block their threads)
        // BUG: This causes a race condition, where BufferManager is not quick enough to clean old
        // buffers up and falsely reports, that it has no more room for new buffers
        // I'm open to suggestions on how to fix this
        // NOTE: look into making a separate listener thread for this
        let handle = tokio::spawn(async move {
            logger::log(
                LogLevel::Normal,
                format!(
                    "Dropping this Internal Buffer (id={id}): {:?}",
                    BUFMAN_GLOB.write().await.get_buf(layer, id)
                )
                .as_str(),
            )
            .await;
            BUFMAN_GLOB
                .write()
                .await
                .rem_buf(layer.into(), id)
                .await
                .expect(
                    format!(
                    "BUG: ClientBuffer ({id}) has an invalid ID! Tried to access on layer {layer}"
                )
                    .as_str(),
                );
            logger::log(LogLevel::Normal, format!("Dropped buffer {id}").as_str()).await;
        });
        // let _ = block_on(handle);
    }
}

#[derive(Debug)]
pub struct BufferBorder {
    border_shown: u8,      // xxxx LTDR
    pub corner: [char; 4], // clockwise, starting at top-left
    pub hborder: &'static str,
    pub vborder: char,
    pub lpad: u16,
    pub rpad: u16,
    pub tpad: u16,
    pub dpad: u16,
}

impl BufferBorder {
    /// creates a Border config with the *chosen* defaults
    pub fn new(
        border_shown: u8,
        corner: [char; 4],
        hborder: &'static str,
        vborder: char,
        lpad: u16,
        rpad: u16,
        tpad: u16,
        dpad: u16,
    ) -> Self {
        BufferBorder {
            border_shown,
            corner,
            hborder,
            vborder,
            lpad,
            rpad,
            tpad,
            dpad,
        }
    }
    pub fn blank() -> BufferBorder {
        BufferBorder {
            border_shown: 0xF,
            corner: [CORNER; 4],
            hborder: HBORDER,
            vborder: VBORDER,
            lpad: 0,
            rpad: 0,
            tpad: 0,
            dpad: 0,
        }
    }
    pub fn default() -> BufferBorder {
        BufferBorder {
            border_shown: 0xF,
            corner: ['╭', '╮', '╯', '╰'],
            hborder: HBORDER,
            vborder: VBORDER,
            lpad: 1,
            rpad: 1,
            tpad: 1,
            dpad: 1,
        }
    }
    fn toggle_border(&mut self, border_idx: u8) {
        self.border_shown ^= 1 << border_idx;
    }
    fn show_border(&mut self, show: bool, border_idx: u8) {
        if show != ((self.border_shown >> border_idx & 1) > 0) {
            self.toggle_border(border_idx);
        }
    }
    pub fn toggle_right(&mut self) {
        self.toggle_border(0);
    }
    pub fn toggle_down(&mut self) {
        self.toggle_border(1);
    }
    pub fn toggle_top(&mut self) {
        self.toggle_border(2);
    }
    pub fn toggle_left(&mut self) {
        self.toggle_border(3);
    }
    pub fn showr(&mut self, show: bool) {
        self.show_border(show, 0);
    }
    pub fn showd(&mut self, show: bool) {
        self.show_border(show, 1);
    }
    pub fn showt(&mut self, show: bool) {
        self.show_border(show, 2);
    }
    pub fn showl(&mut self, show: bool) {
        self.show_border(show, 3);
    }
    pub fn show_all(&mut self, show: bool) {
        if show {
            self.border_shown = 0xF;
        } else {
            self.border_shown = 0;
        }
    }
    pub fn get_borders_shown(&self) -> [bool; 4] {
        [
            self.border_shown >> 3 & 1 > 0,
            self.border_shown >> 2 & 1 > 0,
            self.border_shown >> 1 & 1 > 0,
            self.border_shown >> 0 & 1 > 0, // >> 0 is just for aesthetics
        ]
    }

    pub fn get_number_of_borders(&self) -> (u16, u16) {
        let hborders = (self.border_shown & 0x06).count_ones() as u16;
        let vborders = (self.border_shown & 0x09).count_ones() as u16;
        (vborders, hborders)
    }
}

#[derive(Debug)]
pub struct Buffer {
    offx: u16,
    offy: u16,
    width: u16,
    height: u16,
    border: Option<BufferBorder>,
    ctrl_codes: Vec<(ANSICode, usize)>,
    content: String,
}

impl Buffer {
    fn new(offx: u16, offy: u16, width: u16, height: u16) -> Buffer {
        Buffer {
            offx,
            offy,
            width,
            height,
            border: Some(BufferBorder::default()),
            ctrl_codes: Vec::new(),
            content: String::new(),
        }
    }
    pub fn ctrl_codes(&self) -> std::slice::Iter<(ANSICode, usize)> {
        self.ctrl_codes.iter()
    }
    pub fn border(&self) -> Option<&BufferBorder> {
        self.border.as_ref()
    }
    pub fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }
    pub fn offsets(&self) -> (u16, u16) {
        (self.offx, self.offy)
    }
    pub fn get_start_of_text(&self) -> (u16, u16) {
        let mut x = self.offx;
        let mut y = self.offy;
        if let Some(b) = self.border.as_ref() {
            x += b.lpad + if b.border_shown >> 3 & 1 > 0 { 1 } else { 0 };
            y += b.tpad + if b.border_shown >> 2 & 1 > 0 { 1 } else { 0 };
        }
        (x, y)
    }

    fn default() -> Self {
        Buffer::new(0, 0, 20, 20)
    }

    fn center(&mut self) {
        let len = self.get_auto_width();
        let mut border = match self.border.take() {
            Some(b) => b,
            None => BufferBorder::blank(),
        };
        border.tpad = (self.height - 1 - self.content.lines().count() as u16) / 2;
        border.lpad = (self.width - len as u16) / 2;
        self.border = Some(border);
    }

    fn get_auto_width(&self) -> usize {
        self.content
            .lines()
            .max_by(|x, y| x.len().cmp(&y.len()))
            .unwrap_or("")
            .len()
    }

    fn auto_size(&mut self) {
        let blank = BufferBorder::blank();
        let BufferBorder {
            lpad,
            rpad,
            tpad,
            dpad,
            ..
        } = self.border.as_ref().unwrap_or(&blank);
        let (width, height) = self.get_auto_size(*lpad, *rpad, *tpad, *dpad);
        self.width = width;
        self.height = height;
    }

    // async fn render_fixed_size(&self) -> std::io::Result<()> {
    //     // if BUFMAN_SINGLETON.lock().await.check_space(self).await {
    //     super::print_lines(&self.to_string_border(&self.border), self.offy, self.offx)?;
    //     Ok(())
    //     // } else {
    //     //     Err(std::io::Error::new(ErrorKind::Other, "Invalid placement!")) // TODO: I hate this
    //     // }
    // }

    // async fn render(&mut self) -> std::io::Result<()> {
    //     let (width, height) = if let BufferBorder::Border {
    //         lpad,
    //         rpad,
    //         tpad,
    //         dpad,
    //         ..
    //     } = self.border
    //     {
    //         self.get_auto_size(lpad, rpad, tpad, dpad)
    //     } else {
    //         self.get_auto_size(0, 0, 0, 0)
    //     };
    //     self.width = width;
    //     self.height = height;
    //     self.render_fixed_size().await
    // }
}

impl Display for Buffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.content)?;
        Ok(())
    }
}

const BITS_PER_EL: usize = 32;
const MAX_VAL_EL: u32 = u32::MAX;
const GAP_CHAR: char = '@';
#[derive(Debug)]
pub struct RenderBuffer {
    data: Vec<char>, // is there something faster than this?
    init_ctrl_codes: Vec<ANSICode>,
    last_ctrl_codes: Vec<ANSICode>,
    ctrl_codes: Vec<(ANSICode, usize)>,
    write_locks: Vec<u32>,
}

use crossterm::execute;
impl RenderBuffer {
    const INIT_CODES_CAP: usize = 0;
    const LAST_CODES_CAP: usize = 1;
    fn new(term_width: u16, term_height: u16) -> Self {
        let chars_cap = (term_width * term_height) as usize;
        let data = vec![GAP_CHAR; chars_cap];
        let write_locks = vec![0; chars_cap / BITS_PER_EL + 1];
        let ctrl_codes = Vec::with_capacity(chars_cap); // worst case
        RenderBuffer {
            data,
            init_ctrl_codes: Vec::with_capacity(RenderBuffer::INIT_CODES_CAP),
            last_ctrl_codes: Vec::with_capacity(RenderBuffer::LAST_CODES_CAP),
            ctrl_codes,
            write_locks,
        }
    }

    fn find_nearest_smaller_pow2(val: u32) -> u32 {
        if val == 0 {
            return 0; // invalid state
        }
        let mut pow = BITS_PER_EL / 2;
        let mut ret = 1 << (BITS_PER_EL - 1);
        let mut increase: bool = ret < val;
        while (ret & val) == 0 {
            ret = if increase { ret << pow } else { ret >> pow };
            increase = ret < val;
            pow /= 2;
        }
        ret
    }

    fn clear(&mut self) {
        // execute!(stdout(), Clear(ClearType::All)).unwrap();
        self.write_locks.fill(0);
    }
    fn fill_rest(&mut self) {
        // importante
        let last_idx = self.write_locks.len() - 1;
        self.write_locks[last_idx] ^=
            MAX_VAL_EL ^ ((1 << (self.data.len() - last_idx * BITS_PER_EL)) - 1);
        for (i, chunk) in self.write_locks.iter().enumerate() {
            let mut chunk = chunk ^ MAX_VAL_EL;
            while chunk > 0 {
                let off = RenderBuffer::find_nearest_smaller_pow2(chunk);
                chunk ^= off;
                let off = (off as f32).log2();
                assert!(off % 1. == 0.);
                self.data[i * BITS_PER_EL + off as usize] = GAP_CHAR;
            }
        }
    }

    #[inline(always)]
    fn conv_idx(x: usize, y: usize, term_width: u16) -> usize {
        x + y * (term_width as usize)
    }
    fn check_lock(&mut self, idx: usize) -> bool {
        let res = self.write_locks[idx / BITS_PER_EL] >> (idx % BITS_PER_EL) & 1;
        if res == 0 {
            self.write_locks[idx / BITS_PER_EL] |= 1 << (idx % BITS_PER_EL);
            true
        } else {
            false
        }
    }
    pub fn add_last_ctrl_code(&mut self, code: ANSICode) {
        self.last_ctrl_codes.push(code);
    }
    pub fn add_init_ctrl_code(&mut self, code: ANSICode) {
        self.init_ctrl_codes.push(code);
    }
    pub async fn add_ctrl_code(&mut self, code: ANSICode, x: usize, y: usize, term_width: u16) {
        self.add_ctrl_code_single_idx((code, RenderBuffer::conv_idx(x, y, term_width)))
            .await;
    }
    pub async fn add_ctrl_code_single_idx(&mut self, code: (ANSICode, usize)) {
        if self.write_locks[code.1 / BITS_PER_EL] >> (code.1 % BITS_PER_EL) & 1 == 0 {
            logger::log(LogLevel::Debug, "Success").await;
            self.ctrl_codes.push(code);
        }
    }
    pub async fn write(&mut self, x: usize, y: usize, term_width: u16, char: char) {
        let idx = RenderBuffer::conv_idx(x, y, term_width);
        if self.check_lock(idx) {
            self.data[idx] = char;
        }
    }
    pub async fn write_str(&mut self, x: usize, y: usize, term_width: u16, str: &str) {
        let mut idx = RenderBuffer::conv_idx(x, y, term_width);
        for char in str.chars() {
            if self.check_lock(idx) {
                self.data[idx] = char;
            }
            idx += 1;
        }
    }

    // flushes a buffer
    async fn flush(&mut self) -> std::io::Result<()> {
        self.fill_rest();
        queue!(stdout(), Clear(ClearType::All)).unwrap();
        self.init_ctrl_codes.iter().try_for_each(|code| {
            stdout().queue(Print(code.conv()))?;
            Ok::<(), std::io::Error>(())
        })?;
        if self.ctrl_codes.len() > 0 {
            self.ctrl_codes.sort_by(|(_, i), (_, j)| j.cmp(i));
            logger::log(
                LogLevel::Debug,
                format!("Control codes: {:?}", self.ctrl_codes).as_str(),
            )
            .await;
            let mut next_code = self.ctrl_codes.pop();
            let mut next_idx = match next_code {
                Some((_, idx)) => idx as isize,
                None => -1,
            };
            self.data.iter().enumerate().try_for_each(|(i, c)| {
                if next_idx == i as isize {
                    let code = next_code.unwrap().0;
                    stdout().queue(Print(code))?;
                    next_code = self.ctrl_codes.pop();
                    next_idx = match next_code {
                        Some((_, idx)) => idx as isize,
                        None => -1,
                    };
                }
                stdout().queue(Print(c))?;
                Ok::<(), std::io::Error>(())
            })?;
        } else {
            self.data.iter().try_for_each(|c| {
                stdout().queue(Print(c))?;
                Ok::<(), std::io::Error>(())
            })?;
        }
        self.last_ctrl_codes.iter().try_for_each(|code| {
            stdout().queue(Print(code.conv()))?;
            Ok::<(), std::io::Error>(())
        })?;
        stdout().flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power2() {
        let val: u32 = 0b010000;
        assert_eq!(RenderBuffer::find_nearest_smaller_pow2(val), 16);
    }
    #[test]
    fn test_power2_sanity_check() {
        let val: u32 = 0b000001;
        assert_eq!(RenderBuffer::find_nearest_smaller_pow2(val), 1);
    }

    #[test]
    fn test_power2_multiple() {
        let val: u32 = 0b010001;
        let res = RenderBuffer::find_nearest_smaller_pow2(val);
        dbg!(res);
        assert!(res == 16 || res == 1);
    }

    #[test]
    fn test_power2_empty() {
        let val: u32 = 0;
        assert_eq!(RenderBuffer::find_nearest_smaller_pow2(val), 0);
    }
}

// async fn render(
//     buffers: Mutex<impl Iterator<Item = &Buffer> + Clone>,
//     render_buf: Arc<Mutex<RenderBuffer>>,
// ) {
//     let buffers = buffers.lock().await;
//     let futs = buffers.clone().into_iter().map(|buf| {
//         let string = buf.to_string_border(&buf.border);
//         let render_buf = render_buf.clone();
//         async move {
//             let mut render_buf = render_buf.lock().await;
//             for (i, line) in string.lines().enumerate() {
//                 render_buf
//                     .write_str(buf.offx as usize, buf.offy as usize + i, line)
//                     .await;
//             }
//         }
//     });
// }
async fn render_internal_faster(
    buffers: impl Iterator<Item = &Buffer> + Send,
    render_buf: &mut RenderBuffer,
) {
    let (term_width, term_height) = terminal::size().unwrap();
    // eprintln!("width = {term_width} height = {term_height}");
    // TODO: make this faster
    for buf in buffers {
        // logger::log(LogLevel::Debug, format!("{:?}", buf).as_str()).await;
        buf.render(term_width, render_buf).await;
    }
}
// async fn render_internal(
//     buffers: impl Iterator<Item = &Buffer> + Send,
//     render_buf: &mut RenderBuffer,
// ) {
//     let (term_width, term_height) = terminal::size().unwrap();
//     // eprintln!("width = {term_width} height = {term_height}");
//     // TODO: make this faster
//     for buf in buffers {
//         logger::log(LogLevel::Debug, format!("{:?}", buf).as_str()).await;
//         let string =
//             buf.to_string_border_full_with_struct(buf.width, buf.height, buf.border.as_ref());
//         assert_eq!(string.len(), ((buf.width + 1) * buf.height) as usize);
//         for (i, line) in string.lines().enumerate() {
//             render_buf
//                 .write_str(buf.offx as usize, buf.offy as usize + i, term_width, line)
//                 .await;
//         }
//     }
// }

#[async_trait]
trait Layout: DowncastSync {
    async fn render(&mut self, render_buf: &mut RenderBuffer);
    async fn add_buf(&mut self, name: BufferId, buf: Buffer) -> Result<BufferId, &str>;
    async fn rem_buf(&mut self, name: BufferId) -> Result<Buffer, &'static str>; // now this should never be
    fn get_buf(&self, name: BufferId) -> Result<&Buffer, &'static str>;
    fn get_buf_mut(&mut self, name: BufferId) -> Result<&mut Buffer, &str>;
    fn is_full(&self) -> bool;
    fn get_next_focused(&self) -> Option<BufferId>;
}
impl_downcast!(sync Layout);

mod builtin_layouts;
use builtin_layouts::MasterLayout;

struct BufferManager {
    render_buf: RenderBuffer,
    tiled_layouts: Vec<usize>,
    free_layouts: Vec<usize>,
    layers: Vec<Box<dyn Layout>>,
    focused: Option<BufferRef>,
    term_width: u16,
    term_height: u16,
}

/// this is completely safe, since the editor should never run without being able to query the
/// terminal size
pub fn size() -> (u16, u16) {
    terminal::size().expect("Couldn't fetch terminal size!")
}

type DynLayout = Box<dyn Layout + Send + Sync>;

// TODO: the most ideal way to make the public API here would be to have some assoc fns, that
// handle the lock obtaining stuff, instead of direct method calls
impl BufferManager {
    fn new() -> BufferManager {
        let (term_width, term_height) = size();
        let mut layers = Vec::with_capacity(2);
        let ml: Box<dyn Layout> = Box::new(MasterLayout::new());
        layers.push(ml);
        BufferManager {
            render_buf: RenderBuffer::new(term_width, term_height),
            tiled_layouts: vec![0],   // TODO: not final
            free_layouts: Vec::new(), // TODO: not final
            layers,
            focused: None,
            term_width,
            term_height,
        }
    }

    async fn rerender(&mut self) -> std::io::Result<()> {
        self.render_buf.clear();
        logger::log(LogLevel::Normal, "cleared render_buf bitmap").await;
        for i in self.layers.len() - 1..=0 {
            logger::log(LogLevel::Normal, format!("rendering layer {i}...").as_str()).await;
            self.layers[i].render(&mut self.render_buf).await;
        }
        logger::log(LogLevel::Normal, "finish rendering layers").await;
        self.render_buf.flush().await?;
        logger::log(LogLevel::Normal, "finish rerendering").await;
        Ok(())
    }

    fn add_tiled_layer(&mut self, layout: DynLayout) {
        self.tiled_layouts.push(self.layers.len());
        self.add_layer(layout);
    }
    fn add_free_layer(&mut self, layout: DynLayout) {
        self.free_layouts.push(self.layers.len());
        self.add_layer(layout);
    }

    fn add_layer(&mut self, layout: DynLayout) {
        self.layers.push(layout);
    }

    // TODO: make it so, that you can optionally switch focus on buffer add
    async fn add_new_buf(&mut self, layer: u8, id: BufferId) -> Result<BufferId, &str> {
        self.add_buf(layer, id, Buffer::default()).await
    }
    async fn add_buf(&mut self, layer: u8, id: BufferId, buf: Buffer) -> Result<BufferId, &str> {
        let layer = layer as usize;
        if layer >= self.layers.len() {
            // error handling is now a thing
            return Err("Overflow!");
        }
        let res = self.layers[layer].add_buf(id, buf).await;
        // if let Ok(id) = res {
        //     self.focused = Some(self.layers[layer].get_buf(id).unwrap());
        // }
        res
    }

    async fn rem_buf(&mut self, layer: usize, id: BufferId) -> Result<Buffer, &str> {
        let layer = layer as usize;
        if layer >= self.layers.len() {
            // error handling is now a thing
            return Err("Overflow!");
        }
        let res = self.layers[layer].rem_buf(id).await;
        if res.is_ok() {
            match self.layers[layer].get_next_focused() {
                Some(id) => {
                    self.focused = Some(BufferRef {
                        layer: layer as u8,
                        id,
                    })
                }
                None => {}
            }
        }
        res
    }

    async fn change_focus(&mut self, bufman_ref: BufferRef) -> Result<(), &'static str> {
        let buf = self.get_buf(bufman_ref.layer, bufman_ref.id)?;
        let (x, y) = buf.get_start_of_text();
        self.focused = Some(bufman_ref);
        self.render_buf
            .add_last_ctrl_code(ANSICode::SetCursor(x, y));
        Ok(())
    }

    fn get_buf(&self, layer: u8, id: BufferId) -> Result<&Buffer, &'static str> {
        self.layers[layer as usize].get_buf(id)
    }
    fn get_buf_mut(&mut self, layer: u8, id: BufferId) -> Result<&mut Buffer, &str> {
        self.layers[layer as usize].get_buf_mut(id)
    }

    fn get_focused(&self) -> Result<&Buffer, &'static str> {
        if let Some(BufferRef { layer, id }) = self.focused {
            return self.get_buf(layer, id);
        }
        Err("no focused buffer")
    }

    async fn resize(&mut self) -> std::io::Result<()> {
        let (w, h) = terminal::size().unwrap();
        self.term_width = w;
        self.term_height = h;
        self.rerender().await
    }
}

use std::time::{Duration, Instant};

pub async fn bench(buffers: usize) -> Duration {
    let now = Instant::now();
    let mut buffer_vec = Vec::with_capacity(buffers);
    for _ in 0..buffers {
        let mut buf = ClientBuffer::build(0, true).await;
        // bandaid until we figure something for the Drop bug out
        while let Err(_) = buf {
            buf = ClientBuffer::build(0, true).await;
        }
        let buf = buf.unwrap();
        buffer_vec.push(buf);
        let _ = buffer_vec
            .last_mut()
            .unwrap()
            .set_content("test".to_string())
            .await;
    }
    now.elapsed()
    // println!("Elapsed: {:.2?}", elapsed);
}
