use std::io::SeekFrom;

use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
};

use super::{
    logger::{self, LogLevel},
    render::ClientBuffer,
};

pub async fn read(reader: impl AsyncReadExt, start_size: usize) -> std::io::Result<String> {
    let mut ret = String::with_capacity(start_size);
    let mut reader = Box::pin(reader);
    reader.read_to_string(&mut ret).await?;
    Ok(ret)
}

pub async fn read_n_bytes(
    reader: impl AsyncReadExt,
    bytes_to_read: usize,
) -> std::io::Result<Vec<u8>> {
    let mut ret = Vec::with_capacity(bytes_to_read);
    let mut reader = Box::pin(reader);
    reader.read_exact(&mut ret).await?;
    Ok(ret)
}

pub async fn read_file(file_name: &str) -> std::io::Result<String> {
    let file = File::open(file_name).await?;
    let size = file.metadata().await.unwrap().len() as usize;
    read(file, size).await
}

pub async fn read_n_bytes_from_file(
    file_name: &str,
    off: u64,
    bytes_to_read: usize,
) -> std::io::Result<Vec<u8>> {
    let mut file = File::open(file_name).await?;
    file.seek(SeekFrom::Start(off)).await?;
    read_n_bytes(file, bytes_to_read).await
}

pub async fn open_file(file_name: &str) -> std::io::Result<ClientBuffer> {
    let mut c = ClientBuffer::build_on_tiled(2).await;
    while let Err(_) = c {
        // this is life now
        c = ClientBuffer::build_on_tiled(2).await;
    }
    let c = c.unwrap();
    if let Err(msg) = c.set_content(read_file(file_name).await?).await {
        logger::log(LogLevel::Error, &msg).await;
        return Err(std::io::ErrorKind::Other.into());
    }
    Ok(c)
}

// pub async fn write<T>(writer: impl AsyncWriteExt, buf: impl Buf) -> std::io::Result<()> {
//     let mut writer = Box::pin(writer);
//     writer.write_all_buf(&mut buf);
//     Ok(())
// }
