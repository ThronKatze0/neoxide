use crate::core::editor::{Buffer as MotionBuffer, CursorPosition};
use crate::core::event_handling::{EventCallback, EventHandler};
use crate::core::logger::{self, LogLevel};
use std::ops::{Deref, DerefMut};

use super::border::{PrintBorder, CORNER, HBORDER, VBORDER};
use async_trait::async_trait;
use crossterm::cursor::MoveTo;
use crossterm::style::Print;
use crossterm::terminal::{Clear, ClearType};
use crossterm::{queue, terminal, ExecutableCommand, QueueableCommand};
use downcast_rs::{impl_downcast, DowncastSync};
use futures::executor::block_on;
use once_cell::sync::Lazy;
use std::io::{stdout, Write};
use std::ops::Range;
use std::sync::Arc;
use std::{collections::HashMap, fmt::Display};
use strum_macros::EnumCount;
use tokio::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

// NOTE: putting one big lock on the entire buffer manager could hurt performance, look into making
// one lock per layer instead
static BUFMAN_GLOB: Lazy<RwLock<BufferManager>> = Lazy::new(|| RwLock::new(BufferManager::new()));
static RENDER_EVH: Lazy<EventHandler<Event, EventData>> = Lazy::new(|| {
    let evh = EventHandler::new();
    let callback: EventCallback<Event, _> = EventCallback::new(
        Arc::new(Box::new(|_: Arc<Mutex<_>>| {
            let fut = async { bufman_read().await.resize().await.unwrap() };
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

pub async fn dispatch_resize() {
    RENDER_EVH
        .dispatch(Event::Resize, Arc::new(Mutex::new(EventData)))
        .await;
}

pub struct DirectBufferReference<'a>(MutexGuard<'a, Box<dyn Layout>>, BufferRef);
pub trait ContentRef {
    fn content(&self) -> &Vec<String>;
}

impl<'a> ContentRef for DirectBufferReference<'a> {
    fn content(&self) -> &Vec<String> {
        &self.content
    }
}
impl<'a> Deref for DirectBufferReference<'a> {
    type Target = Buffer;
    fn deref(&self) -> &Self::Target {
        self.0
            .get_buf(self.1.id)
            .expect("FocusedBuffer has no associated buffer struct! This should never happen!")
    }
}
impl<'a> DerefMut for DirectBufferReference<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.get_buf_mut(self.1.id).unwrap()
    }
}

pub struct PublicBufferReference<'a>(RwLockReadGuard<'a, BufferManager>, BufferRef);
impl<'a> PublicBufferReference<'a> {
    pub async fn deref(&self) -> DirectBufferReference {
        self.0
            .get_buf(self.1.layer, self.1.id)
            .await
            .expect("Orphaned Buffer Reference!")
    }
}

pub async fn focused<'a>() -> Result<PublicBufferReference<'a>, &'static str> {
    let handle = bufman_read().await;
    logger::log(LogLevel::Debug, "Got handle!").await;
    if handle.focused.is_none() {
        return Err("no focus");
    }
    let buf_ref = handle.focused.clone().unwrap();
    Ok(PublicBufferReference(handle, buf_ref))
    // handle.get_buf(buf_ref.layer, buf_ref.id).await
    // let lock = handle.layers[buf_ref.layer.clone() as usize].lock().await;
    // Ok(DirectBufferReference(lock, buf_ref))
}

pub async fn update_cursor_pos(new_pos: CursorPosition) {
    let res = {
        match bufman_read().await.focused.clone() {
            Some(buf_ref) => match bufman_read()
                .await
                .get_buf_mut(buf_ref.layer, buf_ref.id)
                .await
            {
                Ok(mut buf) => {
                    buf.set_cursor_pos(new_pos);
                    Ok(())
                }
                Err(msg) => Err(format!("error updating values: {msg}")),
            },
            None => Err("no focus".to_string()),
        }
    };
    if let Err(msg) = res {
        logger::log(
            LogLevel::Error,
            format!("Failed to set cursor pos: {msg}").as_str(),
        )
        .await;
    }
}

fn set_cursor(x: u16, y: u16) -> std::io::Result<()> {
    stdout().execute(MoveTo(x, y))?;
    Ok(())
}

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
    // Look into splitting this into multiple fns in the future
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
    pub async fn focus(&self) -> Result<(), String> {
        bufman_write()
            .await
            .change_focus(self.bufman_ref.clone())
            .await?;

        let (x, y) = bufman_read()
            .await
            .get_buf(self.layer(), self.id())
            .await
            .unwrap()
            .get_start_of_text();
        set_cursor(x, y).map_err(|err| err.to_string())?;

        //if let Err(err) = set_cursor(x as u16, y as u16) {
        //    return Err(format!("set_cursor: {}", err.to_string()));
        //}
        Ok(())
    }
    #[inline]
    fn id(&self) -> BufferId {
        self.bufman_ref.id
    }
    #[inline]
    fn layer(&self) -> u8 {
        self.bufman_ref.layer
    }

    pub async fn set_color(&mut self, range: Range<usize>, color: ANSICode) {
        let handle = bufman_read().await;

        let mut buf = handle.get_buf_mut(self.layer(), self.id()).await.expect(
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
        let handle = bufman_read().await;
        let BufferRef { layer, id } = self.bufman_ref;
        let mut buf = handle.get_buf_mut(layer, id).await?;
        buf.content = content.lines().map(|str| str.to_string()).collect(); // TODO: make better
        drop(buf);
        logger::log(LogLevel::Normal, "start rerendering").await;
        if let Err(err) = handle.rerender().await {
            return Err(format!("Error when rerendering: {err}"));
        }
        logger::log(LogLevel::Normal, "finish rerendering (for realz)").await;
        Ok(())
    }
    pub async fn build(id: BufferId, tiled: bool) -> Result<Self, String> {
        let handle = bufman_read().await;
        let vec = if tiled {
            &handle.tiled_layouts
        } else {
            &handle.free_layouts
        }
        .read()
        .await;
        for layer in vec.iter().map(|layer| *layer) {
            if !handle.layers[layer].lock().await.is_full() {
                let layer = layer as u8;
                let id = handle.add_new_buf(layer, id).await?;
                logger::log(LogLevel::Debug, "Buffer created!").await;
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
        let mut handle = bufman_write().await;
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
    async fn get_pbr(&self) -> PublicBufferReference {
        PublicBufferReference(bufman_read().await, self.bufman_ref.clone())
    }
    pub async fn get_content(&self) -> PublicBufferReference<'_> {
        self.get_pbr().await
    }
    pub async fn dims(&self) -> impl BufferDims + use<'_> {
        self.get_pbr().await
    }

    // BUG: on tiled layouts, this function yields different results, depending on when it is called. Rewrite this to not do that, as well as add more functionality (anchor content to any corner, etc.)
    pub async fn center(&self) {
        bufman_write()
            .await
            .get_buf_mut(self.layer(), self.id())
            .await
            .expect("Orphaned Clientbuffer!")
            .center()
    }
}

#[async_trait]
pub trait BufferDims {
    async fn width(&self) -> u16;
    async fn height(&self) -> u16;
    async fn offx(&self) -> u16;
    async fn offy(&self) -> u16;
    async fn tpad(&self) -> u16;
    async fn dpad(&self) -> u16;
    async fn lpad(&self) -> u16;
    async fn rpad(&self) -> u16;
}

#[async_trait]
impl BufferDims for PublicBufferReference<'_> {
    async fn dpad(&self) -> u16 {
        self.deref().await.dpad().await
    }
    async fn tpad(&self) -> u16 {
        self.deref().await.tpad().await
    }
    async fn lpad(&self) -> u16 {
        self.deref().await.lpad().await
    }
    async fn rpad(&self) -> u16 {
        self.deref().await.rpad().await
    }
    async fn offx(&self) -> u16 {
        self.deref().await.offx().await
    }
    async fn offy(&self) -> u16 {
        self.deref().await.offy().await
    }
    async fn width(&self) -> u16 {
        self.deref().await.width().await
    }
    async fn height(&self) -> u16 {
        self.deref().await.height().await
    }
}
const BLANK_BORDER: BufferBorder = BufferBorder::blank();
#[async_trait]
impl BufferDims for DirectBufferReference<'_> {
    async fn dpad(&self) -> u16 {
        self.border.as_ref().unwrap_or(&BLANK_BORDER).dpad
    }
    async fn tpad(&self) -> u16 {
        self.border.as_ref().unwrap_or(&BufferBorder::blank()).tpad
    }
    async fn lpad(&self) -> u16 {
        self.border.as_ref().unwrap_or(&BufferBorder::blank()).lpad
    }
    async fn rpad(&self) -> u16 {
        self.border.as_ref().unwrap_or(&BufferBorder::blank()).rpad
    }
    async fn offx(&self) -> u16 {
        self.offx
    }
    async fn offy(&self) -> u16 {
        self.offy
    }
    async fn width(&self) -> u16 {
        self.width
    }
    async fn height(&self) -> u16 {
        self.height
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
        // Current Plan: Wait until AsyncDrop trait arrives in Rust
        // NOTE: look into making a separate listener thread for this
        let _handle = tokio::spawn(async move {
            logger::log(
                LogLevel::Normal,
                format!(
                    "Dropping this Internal Buffer (id={id}): {:?}",
                    *bufman_read().await.get_buf(layer, id).await.unwrap()
                )
                .as_str(),
            )
            .await;
            bufman_write().await.rem_buf(layer.into(), id).await.expect(
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
    pub const fn blank() -> BufferBorder {
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
    cursor_pos: CursorPosition,
    content: Vec<String>,
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
            cursor_pos: CursorPosition { x: 0, y: 0 },
            content: Vec::new(),
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
    pub fn lines(&self) -> impl Iterator<Item = &String> {
        self.content.iter()
    }
    pub fn set_cursor_pos(&mut self, mut new_pos: CursorPosition) {
        self.cursor_pos = new_pos;
        let (offx, offy) = self.get_start_of_text();
        new_pos.x += offx as u32;
        new_pos.y += offy as u32;
        set_cursor(new_pos.x as u16, new_pos.y as u16);
    }
    pub fn cursor_position(&self) -> CursorPosition {
        self.cursor_pos
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
        border.tpad = (self.height - 1 - self.content.len() as u16) / 2;
        border.lpad = (self.width - len as u16) / 2;
        self.border = Some(border);
    }

    fn get_auto_width(&self) -> usize {
        self.content
            .iter()
            .max_by(|x, y| x.len().cmp(&y.len()))
            .unwrap_or(&String::new())
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
        for line in self.content.iter() {
            write!(f, "{}\n", line)?;
        }
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
    logger::log(LogLevel::Normal, "start rendering buffers...").await;
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
    async fn add_buf(&mut self, name: BufferId, buf: Buffer) -> Result<BufferId, &'static str>;
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
    render_buf: Mutex<RenderBuffer>,
    tiled_layouts: RwLock<Vec<usize>>,
    free_layouts: RwLock<Vec<usize>>,
    layers: Vec<Mutex<Box<dyn Layout>>>,
    focused: Option<BufferRef>,
    term_size: Mutex<(u16, u16)>, // (width, height)
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
        let term_size = size();
        let mut layers = Vec::with_capacity(2);
        let ml: Box<dyn Layout> = Box::new(MasterLayout::new());
        layers.push(Mutex::new(ml));
        BufferManager {
            render_buf: Mutex::new(RenderBuffer::new(term_size.0, term_size.1)),
            tiled_layouts: RwLock::new(vec![0]), // TODO: not final
            free_layouts: RwLock::new(Vec::new()), // TODO: not final
            layers,
            focused: None,
            term_size: Mutex::new(term_size),
        }
    }

    async fn rerender(&self) -> std::io::Result<()> {
        let mut render_buf = self.render_buf.lock().await;
        render_buf.clear();
        logger::log(LogLevel::Normal, "cleared render_buf bitmap").await;
        for i in self.layers.len() - 1..=0 {
            logger::log(LogLevel::Normal, format!("rendering layer {i}...").as_str()).await;
            self.layers[i].lock().await.render(&mut render_buf).await;
        }
        logger::log(LogLevel::Normal, "finish rendering layers").await;
        render_buf.flush().await?;
        logger::log(LogLevel::Normal, "finish rerendering").await;
        Ok(())
    }

    async fn add_tiled_layer(&mut self, layout: DynLayout) {
        self.tiled_layouts.write().await.push(self.layers.len());
        self.add_layer(layout);
    }
    async fn add_free_layer(&mut self, layout: DynLayout) {
        self.free_layouts.write().await.push(self.layers.len());
        self.add_layer(layout);
    }

    fn add_layer(&mut self, layout: DynLayout) {
        self.layers.push(Mutex::new(layout));
    }

    // TODO: make it so, that you can optionally switch focus on buffer add
    async fn add_new_buf(&self, layer: u8, id: BufferId) -> Result<BufferId, &'static str> {
        self.add_buf(layer, id, Buffer::default()).await
    }
    async fn add_buf(
        &self,
        layer: u8,
        id: BufferId,
        buf: Buffer,
    ) -> Result<BufferId, &'static str> {
        let layer = layer as usize;
        if layer >= self.layers.len() {
            // error handling is now a thing
            return Err("Overflow!");
        }
        let res = self.layers[layer].lock().await.add_buf(id, buf).await;
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
        let res = self.layers[layer].lock().await.rem_buf(id).await;
        if res.is_ok() {
            match self.layers[layer].lock().await.get_next_focused() {
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
        self.focused = Some(bufman_ref.clone());
        let buf = self.get_buf(bufman_ref.layer, bufman_ref.id).await?;
        let (x, y) = buf.get_start_of_text();
        self.render_buf
            .lock()
            .await
            .add_last_ctrl_code(ANSICode::SetCursor(x, y));
        Ok(())
    }

    async fn get_buf(
        &self,
        layer: u8,
        id: BufferId,
    ) -> Result<DirectBufferReference, &'static str> {
        let lock = self.layers[layer as usize].lock().await;
        lock.get_buf(id)?;
        Ok(DirectBufferReference(lock, BufferRef { layer, id }))
    }
    async fn get_buf_mut(&self, layer: u8, id: BufferId) -> Result<DirectBufferReference, &str> {
        let lock = self.layers[layer as usize].lock().await;
        lock.get_buf(id)?;
        Ok(DirectBufferReference(lock, BufferRef { layer, id }))
    }

    async fn get_focused(&self) -> Result<DirectBufferReference, &'static str> {
        if let Some(BufferRef { layer, id }) = self.focused {
            return self.get_buf(layer, id).await;
        }
        Err("no focused buffer")
    }

    async fn resize(&self) -> std::io::Result<()> {
        let (w, h) = terminal::size().unwrap();
        let mut lock = self.term_size.lock().await;
        lock.0 = w;
        lock.1 = h;
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
}
