///
/// win_can.rs
///
/// Implementation of CanInterface for Windows using pipes.
/// Will require an existing pipe server to be connected to a CAN port using the 'win_can_utils' package.
///
use crate::{CanInterface, can::CanFrame};
use bincode;
use std::io::{Error as IoError, ErrorKind};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};

pub struct WindowsCan {
    reader: Option<BufReader<NamedPipeClient>>,
    writer: Option<NamedPipeClient>,
}

impl CanInterface for WindowsCan {
    /// Open a CAN device
    ///
    /// Can device is usually attached to a serial COM port (i.e. COM5). This method will open two separate pipes for reading and writing.
    fn open(channel: &str) -> tokio::io::Result<Self> {
        let sanitized = channel
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect::<String>();
        let out_pipe_name = format!(r"\\.\pipe\can_{}_out", sanitized);
        let in_pipe_name = format!(r"\\.\pipe\can_{}_in", sanitized);

        let out_pipe = ClientOptions::new().open(&out_pipe_name)?;
        let in_pipe = ClientOptions::new().open(&in_pipe_name)?;

        Ok(Self {
            reader: Some(BufReader::new(out_pipe)),
            writer: Some(in_pipe),
        })
    }

    async fn read_frame(&mut self) -> tokio::io::Result<CanFrame> {
        let reader = match &mut self.reader {
            Some(r) => r,
            None => {
                return Err(IoError::new(
                    ErrorKind::InvalidData,
                    "No read pipe has been opened",
                ));
            }
        };

        let mut buf = Vec::with_capacity(1000);
        let num_bytes = reader.read_buf(&mut buf).await?;
        if num_bytes == 0 {
            return Err(IoError::new(
                ErrorKind::UnexpectedEof,
                "Pipe closed. EOF was reached (closed connection) or buffer was full",
            ));
        }

        match bincode::serde::decode_from_slice::<CanFrame, _>(&buf, bincode::config::standard()) {
            Ok((frame, _)) => Ok(frame),
            Err(e) => Err(IoError::new(ErrorKind::Other, e)),
        }
    }

    async fn write_frame(&mut self, frame: CanFrame) -> tokio::io::Result<()> {
        let writer = match &mut self.writer {
            Some(r) => r,
            None => {
                return Err(IoError::new(
                    ErrorKind::InvalidData,
                    "No write pipe has been opened",
                ));
            }
        };

        match bincode::serde::encode_to_vec(frame, bincode::config::standard()) {
            Ok(data) => {
                writer.write_all(&data).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
                Ok(())
            }
            Err(e) => Err(IoError::new(ErrorKind::Other, e)),
        }
    }
}

impl WindowsCan {
    /// Open a read-only CAN device
    ///
    /// Can device is usually attached to a serial COM port (i.e. COM5). This method will a single pipe for reading CAN messages. Attempting to write to the port later will throw an InvalidData error.
    pub fn open_read_only(channel: &str) -> tokio::io::Result<Self> {
        let sanitized = channel
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect::<String>();
        let out_pipe_name = format!(r"\\.\pipe\can_{}_out", sanitized);

        let out_pipe = ClientOptions::new().open(&out_pipe_name)?;

        Ok(Self {
            reader: Some(BufReader::new(out_pipe)),
            writer: None,
        })
    }

    /// Open a write-only CAN device
    ///
    /// Can device is usually attached to a serial COM port (i.e. COM5). This method will a single pipe for writing CAN messages. Attempting to read from the port later will throw an InvalidData error.
    pub fn open_write_only(channel: &str) -> tokio::io::Result<Self> {
        let sanitized = channel
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect::<String>();
        let in_pipe_name = format!(r"\\.\pipe\can_{}_in", sanitized);

        let in_pipe = ClientOptions::new().open(&in_pipe_name)?;

        Ok(Self {
            reader: None,
            writer: Some(in_pipe),
        })
    }
}
