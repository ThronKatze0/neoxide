use super::*;
use crate::core::render::manager::render_internal;
struct FloatingLayout {
    buffers: HashMap<BufferId, Buffer>,
}

pub struct MasterLayout {
    master_id: BufferId,
    top_key: BufferId,
    master: Option<Buffer>,
    split_width: u16,
    buffers: HashMap<BufferId, Buffer>,
}

impl MasterLayout {
    pub fn new() -> Self {
        MasterLayout {
            master_id: u32::MAX,
            top_key: 0,
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
            let mut keys: Vec<BufferId> = self.buffers.keys().map(|k| *k).collect();
            keys.sort();
            for (i, key) in keys.iter().enumerate() {
                let buf = self.buffers.get_mut(key).unwrap();
                if let Some(border) = &mut buf.border {
                    border.showl(false);
                    border.showt(false);
                }
                buf.height = buffer_height;
                buf.width = term_width - self.split_width;
                buf.offx = self.split_width;
                buf.offy = i as u16 * buf.height;
            }

            // self.buffers.values_mut().enumerate().for_each(|(i, buf)| {
            //     let msg = format!("{i}: {buf:?}");
            //     block_on(logger::log(LogLevel::Debug, msg.as_str()));
            //     // futs.push(async move { logger::log(LogLevel::Debug, msg.as_str()).await });
            // });

            // top one needs to have a top border
            if let Some(border) = &mut self.buffers.get_mut(&keys[0]).unwrap().border {
                border.toggle_top();
            }

            // make sure the last buffer takes up all the remaining space
            self.buffers.get_mut(&keys[keys.len() - 1]).unwrap().height =
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

    fn get_id(&mut self) -> BufferId {
        let ret = self.top_key;
        self.top_key += 1;
        ret
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

    async fn add_buf(&mut self, _name: BufferId, buf: Buffer) -> Result<BufferId, &str> {
        let name = self.get_id();
        if self.master.is_none() {
            self.master = Some(buf);
            self.master_id = name;
        } else if let None = self.buffers.get(&name) {
            self.buffers.insert(name, buf);
        } else {
            return Err("duplicate");
        }
        self.reorder().await;
        return Ok(name);
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
