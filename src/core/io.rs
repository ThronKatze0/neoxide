use std::io::SeekFrom;

use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
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

// pub async fn write<T>(writer: impl AsyncWriteExt, buf: impl Buf) -> std::io::Result<()> {
//     let mut writer = Box::pin(writer);
//     writer.write_all_buf(&mut buf);
//     Ok(())
// }
