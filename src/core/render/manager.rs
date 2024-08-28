use crate::core::logger::{self, LogLevel};

use super::border::{PrintBorder, CORNER, HBORDER, VBORDER};
use crate::core::editor::Buffer as MotionBuffer;
use async_trait::async_trait;
use crossterm::cursor::MoveTo;
use crossterm::style::Print;
use crossterm::terminal::{Clear, ClearType};
use crossterm::{queue, terminal, QueueableCommand};
use once_cell::sync::Lazy;
use std::io::{stdout, Write};
use std::{collections::HashMap, fmt::Display};
use tokio::sync::RwLock;

static BUFMAN_GLOB: Lazy<RwLock<BufferManager>> = Lazy::new(|| RwLock::new(BufferManager::new()));

type BufferId = &'static str;

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
        handle.rerender().await.unwrap();
        logger::log(LogLevel::Normal, "finish rerendering (for realz)").await;
        Ok(())
    }
    pub async fn build(layer: u8, id: BufferId) -> Result<Self, String> {
        BUFMAN_GLOB.write().await.add_new_buf(layer, id).await?;
        Ok(ClientBuffer { layer, id })
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
            .unwrap()
            .content
            .lines()
            .map(|str| str.to_string())
            .collect()
    }
}

impl Drop for ClientBuffer {
    fn drop(&mut self) {
        let layer = self.layer;
        let id = self.id;
        // this is the best i can do for now
        // .blocking_write() crashes the entire program
        tokio::spawn(async move {
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
            corner: [CORNER; 4],
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
    pub fn get_borders_shown(&self) -> (bool, bool, bool, bool) {
        (
            self.border_shown >> 3 & 1 > 0,
            self.border_shown >> 2 & 1 > 0,
            self.border_shown >> 1 & 1 > 0,
            self.border_shown >> 0 & 1 > 0, // >> 0 is just for aesthetics
        )
    }
}

#[derive(Debug)]
struct Buffer {
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

    fn default() -> Self {
        Buffer::new(0, 0, 20, 20)
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
struct RenderBuffer {
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
    async fn conv_idx(x: usize, y: usize, term_width: u16) -> usize {
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
    async fn write_internal(&mut self, idx: usize, char: char) {
        if self.check_lock(idx) {
            // eprintln!("{idx} = {:b}", self.write_locks[idx / BITS_PER_EL]);
            self.data[idx] = char;
        }
    }
    async fn write(&mut self, x: usize, y: usize, term_width: u16, char: char) {
        let idx = RenderBuffer::conv_idx(x, y, term_width).await;
        self.write_internal(idx, char).await;
    }
    async fn write_str(&mut self, x: usize, y: usize, term_width: u16, str: &str) {
        let idx = RenderBuffer::conv_idx(x, y, term_width).await;
        for (i, char) in str.chars().enumerate() {
            let idx = i + idx;
            self.write_internal(idx, char).await; // this should be fine, since the checks
                                                  // should have already happened
        }
    }

    // technically flushes a buffer
    fn flush(&mut self) -> std::io::Result<()> {
        self.fill_rest();
        queue!(stdout(), Clear(ClearType::All), MoveTo(0, 0))?;
        stdout().queue(MoveTo(0, 0))?;
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
async fn render_internal(
    buffers: impl Iterator<Item = &Buffer> + Send,
    render_buf: &mut RenderBuffer,
) {
    let (term_width, term_height) = terminal::size().unwrap();
    eprintln!("width = {term_width} height = {term_height}");
    // TODO: make this faster
    for buf in buffers {
        dbg!(buf);
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
trait Layout {
    async fn render(&mut self, render_buf: &mut RenderBuffer);
    async fn add_buf(&mut self, name: BufferId, buf: Buffer) -> Result<(), &str>;
    async fn rem_buf(&mut self, name: BufferId) -> Result<Buffer, &str>; // now this should never be
    fn get_buf(&self, name: BufferId) -> Result<&Buffer, &str>;
    fn get_buf_mut(&mut self, name: BufferId) -> Result<&mut Buffer, &str>;
}

struct FloatingLayout {
    buffers: HashMap<BufferId, Buffer>,
}

use std::collections::LinkedList;
struct MasterLayout {
    master_id: BufferId,
    master: Option<Buffer>,
    split_width: u16,
    buffers: HashMap<BufferId, Buffer>,
}

impl MasterLayout {
    fn new() -> Self {
        MasterLayout {
            master_id: "",
            master: None,
            split_width: terminal::size().expect("Could not get terminal size!").0 / 2,
            buffers: HashMap::new(),
        }
    }
    async fn reorder(&mut self) {
        let len = self.buffers.len() as u16;
        let (term_width, term_height) = terminal::size()
            .expect("Masterlayout reorder function couldn't get size! This should be impossible");

        let mut master = self.master.take().expect("BUG: master field not set");
        (master.offx, master.offy) = (0, 0);
        master.width = if self.buffers.len() > 0 {
            self.split_width
        } else {
            term_width
        };
        if let Some(border) = &mut master.border {
            border.show_all(true);
        }
        master.height = term_height;
        self.master = Some(master);
        if self.buffers.len() > 0 {
            let buffer_height = term_height / len;
            self.buffers.values_mut().enumerate().for_each(|(i, buf)| {
                if let Some(border) = &mut buf.border {
                    border.showl(false);
                    border.showt(false);
                }
                buf.height = buffer_height;
                buf.width = term_width - self.split_width;
                buf.offx = self.split_width;
                buf.offy = i as u16 * buf.height;
            });

            // top one needs to have a top border
            if let Some(border) = &mut self.buffers.values_mut().next().unwrap().border {
                border.toggle_top();
            }

            // make sure the last buffer takes up all the remaining space
            self.buffers.values_mut().last().unwrap().height =
                term_height - buffer_height * (self.buffers.len() - 1) as u16;
        }

        // BUFMAN_GLOB.write().await.rerender().await
    }

    pub fn change_master(&mut self, new_master_id: BufferId) -> Result<(), &str> {
        match self.master.take() {
            Some(master) => match self.buffers.try_insert(self.master_id, master) {
                Ok(_) => match self.buffers.remove(&new_master_id) {
                    Some(new_master) => {
                        self.master = Some(new_master);
                        self.master_id = new_master_id;
                        Ok(())
                    }
                    None => Err("New master not found!"),
                },
                Err(_) => Err("Master already in buffers?"),
            },
            None => panic!("BUG: master field not set"),
        }
    }
}

#[async_trait]
impl Layout for MasterLayout {
    fn get_buf(&self, name: BufferId) -> Result<&Buffer, &str> {
        if self.master_id == name {
            return Ok(self.master.as_ref().unwrap());
        }
        match self.buffers.get(&name) {
            Some(buf) => Ok(buf),
            None => Err("not found"),
        }
    }
    fn get_buf_mut(&mut self, name: BufferId) -> Result<&mut Buffer, &str> {
        if self.master_id == name {
            return Ok(self.master.as_mut().unwrap());
        }
        match self.buffers.get_mut(&name) {
            Some(buf) => Ok(buf),
            None => Err("not found"),
        }
    }
    async fn render(&mut self, render_buf: &mut RenderBuffer) {
        if let None = self.master {
            return;
        }
        let mut buffers = vec![self.master.as_ref().unwrap()];
        buffers.extend(self.buffers.values());
        render_internal(buffers.into_iter(), render_buf).await;
    }

    async fn add_buf(&mut self, name: BufferId, buf: Buffer) -> Result<(), &str> {
        if self.master.is_none() {
            self.master = Some(buf);
            self.master_id = name;
        } else if let None = self.buffers.get(&name) {
            self.buffers.insert(name, buf);
        } else {
            return Err("duplicate");
        }
        self.reorder().await;
        return Ok(());
    }
    async fn rem_buf(&mut self, name: BufferId) -> Result<Buffer, &str> {
        let mut reorder = true;
        let res = if self.master_id == name {
            let new_master_id = self.buffers.keys().next();
            match new_master_id {
                Some(key) => {
                    let key = *key;
                    self.master_id = key;
                    let old_master = self.master.take().expect("rem_buf second impossible state");
                    self.master =
                        Some(self.buffers.remove(&key).expect("rem_buf impossible state"));
                    Ok(old_master)
                }
                None => {
                    reorder = false;
                    Ok(self.master.take().expect("rem_buf third impossible state"))
                }
            }
        } else {
            match self.buffers.remove(&name) {
                Some(buf) => Ok(buf),
                None => Err("not found"),
            }
        };
        if res.is_ok() && reorder {
            self.reorder().await;
        }
        res
    }
}

struct BufferManager {
    render_buf: RenderBuffer,
    layers: Vec<Box<dyn Layout + Send + Sync>>,
    term_width: u16,
    term_height: u16,
}

impl BufferManager {
    fn new() -> BufferManager {
        let (term_width, term_height) = terminal::size().expect("Couldn't fetch terminal size!");
        let mut layers = Vec::with_capacity(2);
        let ml: Box<dyn Layout + Send + Sync> = Box::new(MasterLayout::new());
        layers.push(ml);
        BufferManager {
            render_buf: RenderBuffer::new(term_width, term_height),
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

    fn add_layer(&mut self, layout: Box<dyn Layout + Send + Sync>) {
        self.layers.push(layout);
    }

    async fn add_new_buf(&mut self, layer: u8, id: BufferId) -> Result<(), &str> {
        self.add_buf(layer, id, Buffer::default()).await?;
        Ok(())
    }
    async fn add_buf(&mut self, layer: u8, id: BufferId, buf: Buffer) -> Result<(), &str> {
        // TODO: make error handling a thing
        self.layers[layer as usize].add_buf(id, buf).await?;
        Ok(())
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
}
