use tokio::fs::File;
use tokio::io::{AsyncWrite, AsyncWriteExt};

pub enum LogLevel {
    Normal,
    Warning,
    Error,
    FatalError,
    Bug,
    Debug,
}

impl ToString for LogLevel {
    fn to_string(&self) -> String {
        let ret = match self {
            Self::Normal => "LOG",
            Self::Warning => "WARN",
            Self::Error => "ERR",
            Self::FatalError => "FATAL",
            Self::Debug => "DEBUG",
            Self::Bug => "BUG",
        };
        ret.to_string()
    }
}

pub struct Message<'a> {
    level: LogLevel,
    msg: &'a str,
}

pub const LOGFILE_PATH: &str = "./log.neo";
impl<'a> Message<'a> {
    pub fn new(level: LogLevel, msg: &'a str) -> Self {
        Message { level, msg }
    }
    fn format(&self) -> String {
        format!("[{}] {}\n", self.level.to_string(), self.msg)
    }
    pub async fn log_full(&self, mut stream: impl AsyncWrite + Unpin) -> std::io::Result<()> {
        stream.write(self.format().as_bytes()).await?;
        stream.flush().await?;
        Ok(())
    }

    pub async fn log(&self) {
        let logfile = File::options()
            .append(true)
            .create(true)
            .open(LOGFILE_PATH)
            .await
            .unwrap();
        self.log_full(logfile).await.unwrap();
    }
}

pub async fn log(level: LogLevel, msg: &'_ str) {
    Message::new(level, msg).log().await;
}
