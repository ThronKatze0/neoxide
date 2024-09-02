use crate::core::event_handling::{EventCallback, EventHandler};
use crate::core::logger::{self, LogLevel};

use super::border::{PrintBorder, CORNER, HBORDER, VBORDER};
use async_trait::async_trait;
use crossterm::cursor::MoveTo;
use crossterm::style::Print;
use crossterm::terminal::{Clear, ClearType};
use crossterm::{queue, terminal, QueueableCommand};
use downcast_rs::{impl_downcast, DowncastSync};
use futures::executor::block_on;
use once_cell::sync::Lazy;
use std::io::{stdout, Write};
use std::sync::Arc;
use std::{collections::HashMap, fmt::Display};
use strum_macros::EnumCount;
use tokio::sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};

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

type BufferId = u32; // NOTE: just don't create 2^32-1 buffers on one layer
pub struct ClientBuffer {
    layer: u8,
    id: BufferId,
}

const CLIENTBUF_ID_ERR: &str =
    "BUG: ClientBuffer ({id}) has an invalid ID! Tried to access on layer {layer}";

impl ClientBuffer {
    pub async fn set_content(&self, content: String) -> Result<(), String> {
        let mut handle = BUFMAN_GLOB.write().await;
        let buf = handle.get_buf_mut(self.layer, self.id)?;
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
                return Ok(ClientBuffer { layer, id });
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
        self.layer = layer;
        let buf = handle
            .rem_buf(layer.into(), self.id)
            .await
            .expect(CLIENTBUF_ID_ERR);
        if let Err(msg) = handle.add_buf(layer, self.id, buf).await {
            return Err(format!("adding buffer failed: {}", msg));
        }
        Ok(())
    }

    pub async fn get_content(&self) -> Vec<String> {
        BUFMAN_GLOB
            .read()
            .await
            .get_buf(self.layer, self.id)
            .expect(format!("Orphaned Client Buffer: {}/{}", self.layer, self.id).as_str())
            .content
            .lines()
            .map(|str| str.to_string())
            .collect()
    }

    pub async fn center(&self) {
        BUFMAN_GLOB
            .write()
            .await
            .get_buf_mut(self.layer, self.id)
            .expect("Orphaned Clientbuffer!")
            .center()
    }
}

impl Drop for ClientBuffer {
    fn drop(&mut self) {
        let layer = self.layer;
        let id = self.id;
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
            content: String::new(),
        }
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
    write_locks: Vec<u32>,
}

impl RenderBuffer {
    fn new(term_width: u16, term_height: u16) -> Self {
        let chars_cap = (term_width * term_height) as usize;
        let data = vec![GAP_CHAR; chars_cap];
        let write_locks = vec![0; chars_cap / BITS_PER_EL + 1];
        RenderBuffer { data, write_locks }
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
                // dbg!(format!("{:b}", chunk));
                // dbg!(off);
                // dbg!(i);
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
    pub async fn write(&mut self, x: usize, y: usize, term_width: u16, char: char) {
        let idx = RenderBuffer::conv_idx(x, y, term_width);
        if self.check_lock(idx) {
            // eprintln!("{idx} = {:b}", self.write_locks[idx / BITS_PER_EL]);
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
            // should have already happened
        }
    }

    // technically flushes a buffer
    fn flush(&mut self) -> std::io::Result<()> {
        self.fill_rest();
        queue!(stdout(), Clear(ClearType::All), MoveTo(0, 0))?;
        self.data.iter().try_for_each(|c| {
            stdout().queue(Print(c))?;
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
    eprintln!("width = {term_width} height = {term_height}");
    // TODO: make this faster
    for buf in buffers {
        logger::log(LogLevel::Debug, format!("{:?}", buf).as_str()).await;
        buf.render(term_width, render_buf).await;
    }
}
async fn render_internal(
    buffers: impl Iterator<Item = &Buffer> + Send,
    render_buf: &mut RenderBuffer,
) {
    let (term_width, term_height) = terminal::size().unwrap();
    eprintln!("width = {term_width} height = {term_height}");
    // TODO: make this faster
    for buf in buffers {
        logger::log(LogLevel::Debug, format!("{:?}", buf).as_str()).await;
        let string =
            buf.to_string_border_full_with_struct(buf.width, buf.height, buf.border.as_ref());
        assert_eq!(string.len(), ((buf.width + 1) * buf.height) as usize);
        for (i, line) in string.lines().enumerate() {
            render_buf
                .write_str(buf.offx as usize, buf.offy as usize + i, term_width, line)
                .await;
        }
    }
}

#[async_trait]
trait Layout: DowncastSync {
    async fn render(&mut self, render_buf: &mut RenderBuffer);
    async fn add_buf(&mut self, name: BufferId, buf: Buffer) -> Result<BufferId, &str>;
    async fn rem_buf(&mut self, name: BufferId) -> Result<Buffer, &str>; // now this should never be
    fn get_buf(&self, name: BufferId) -> Result<&Buffer, &str>;
    fn get_buf_mut(&mut self, name: BufferId) -> Result<&mut Buffer, &str>;
    fn is_full(&self) -> bool;
}
impl_downcast!(sync Layout);

mod builtin_layouts;
use builtin_layouts::MasterLayout;

struct BufferManager {
    render_buf: RenderBuffer,
    tiled_layouts: Vec<usize>,
    free_layouts: Vec<usize>,
    layers: Vec<Box<dyn Layout>>,
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
        self.render_buf.flush()?;
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

    async fn add_new_buf(&mut self, layer: u8, id: BufferId) -> Result<BufferId, &str> {
        self.add_buf(layer, id, Buffer::default()).await
    }
    async fn add_buf(&mut self, layer: u8, id: BufferId, buf: Buffer) -> Result<BufferId, &str> {
        let layer = layer as usize;
        if layer >= self.layers.len() {
            // error handling is now a thing
            return Err("Overflow!");
        }
        self.layers[layer].add_buf(id, buf).await
    }

    async fn rem_buf(&mut self, layer: usize, id: BufferId) -> Result<Buffer, &str> {
        // TODO: make error handling a thing
        self.layers[layer].rem_buf(id).await
    }

    fn get_buf(&self, layer: u8, id: BufferId) -> Result<&Buffer, &str> {
        self.layers[layer as usize].get_buf(id)
    }
    fn get_buf_mut(&mut self, layer: u8, id: BufferId) -> Result<&mut Buffer, &str> {
        self.layers[layer as usize].get_buf_mut(id)
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
            .last()
            .unwrap()
            .set_content("test".to_string())
            .await;
    }
    now.elapsed()
    // println!("Elapsed: {:.2?}", elapsed);
}
