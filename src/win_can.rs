///
/// win_can.rs
///
/// Implementation of CanInterface for Windows using pipes.
/// Will require an existing pipe server to be connected to a CAN port using the 'win_can_utils' package.
///
use crate::{CanInterface, can::CanFrame};
use bincode;
use serde::{Deserialize, Serialize};
use std::io::{Error as IoError, ErrorKind};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};

// The CanInterface will fail to open a connection to a win_can_utils canserver if it isn't the matching version.
const WIN_CAN_UTILS_TARGET_VERSION: &str = "0.2.0";

pub struct WindowsCan {
    reader: Option<BufReader<NamedPipeClient>>,
    writer: Option<NamedPipeClient>,
    channel: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CanServerConfig {
    pub bitrate: Option<u32>,
    pub version: String,
}

impl CanInterface for WindowsCan {
    /// Open a CAN device
    ///
    /// Can device is usually attached to a serial COM port (i.e. COM5). This method will open two separate pipes for reading and writing.
    async fn open(channel: &str) -> tokio::io::Result<Self> {
        let sanitized = channel
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect::<String>();
        let out_pipe_name = format!(r"\\.\pipe\can_{}_out", sanitized);
        let out_pipe = ClientOptions::new().open(&out_pipe_name)?;

        let in_pipe_name = format!(r"\\.\pipe\can_{}_in", sanitized);
        let in_pipe = ClientOptions::new().open(&in_pipe_name)?;

        let interface = Self {
            reader: Some(BufReader::new(out_pipe)),
            writer: Some(in_pipe),
            channel: sanitized,
        };

        // Check the version number of the win_can_utils package that we are connecting to
        let ver = interface.get_config().await?.version;
        if ver != WIN_CAN_UTILS_TARGET_VERSION.to_string() {
            return Err(IoError::new(
                ErrorKind::InvalidData,
                format!(
                    "Installed win_can_utils is version {:?}. Version {:?} is required.",
                    ver, WIN_CAN_UTILS_TARGET_VERSION
                ),
            ));
        }

        Ok(interface)
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

        // Helper function to check if BufReader.read_exact() is returning zero bytes (issue has occured)
        let check_bytes = |num_bytes: usize| {
            if num_bytes == 0 {
                return Err(IoError::new(
                    ErrorKind::UnexpectedEof,
                    "Pipe closed. EOF was reached (closed connection) or buffer was full",
                ));
            }
            Ok(())
        };

        // Read the length prefix of next CanFrame (always 1 byte)
        let mut len_prefix = [0u8; 1];
        check_bytes(reader.read_exact(&mut len_prefix).await?)?;

        // Read the bytes for the next CanFrame
        let mut buf = vec![0u8; len_prefix[0] as usize];
        check_bytes(reader.read_exact(&mut buf).await?)?;

        // Deserialize CanFrame bytes into struct
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

    async fn get_bitrate(&mut self) -> std::io::Result<Option<u32>> {
        let config = self.get_config().await?;
        Ok(config.bitrate)
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
            channel: sanitized,
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
            channel: sanitized,
        })
    }

    pub async fn get_config(&self) -> std::io::Result<CanServerConfig> {
        // Connect to config pipe
        let config_pipe_name = format!(r"\\.\pipe\can_{}_config_out", self.channel);
        let config_pipe = ClientOptions::new().open(&config_pipe_name)?;
        let mut config_reader = BufReader::new(config_pipe);

        // Read the config struct
        let mut buf = Vec::new();
        config_reader.read_to_end(&mut buf).await?;

        // Deserialize CanFrame bytes into struct
        let config = serde_json::from_slice::<CanServerConfig>(&buf)?;

        Ok(config)
    }
}
