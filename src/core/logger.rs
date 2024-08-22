use tokio::io::{self, AsyncWrite, AsyncWriteExt};

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

impl<'a> Message<'a> {
    pub fn new(level: LogLevel, msg: &'a str) -> Self {
        Message { level, msg }
    }
    fn format(&self) -> String {
        format!("[{}] {}\n", self.level.to_string(), self.msg)
    }
    pub async fn log_full(&self, mut stream: impl AsyncWrite + Unpin) -> std::io::Result<()> {
        stream.write(self.format().as_bytes()).await?;
        Ok(())
    }

    pub async fn log(&self) -> std::io::Result<()> {
        self.log_full(io::stderr()).await?;
        Ok(())
    }
}
