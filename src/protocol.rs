//! Windows CAN service protocol v1. No Rust-specific types occur on the wire.
use crate::can::CanFrame;
use serde::{Deserialize, Serialize};
use std::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const VERSION: u16 = 1;
pub const MAX_MESSAGE: usize = 65_536;
pub const MAX_QUEUE: usize = 65_536;
pub const MAX_FILTERS: usize = 256;

pub fn validate_name(name: &str) -> io::Result<()> {
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Interface names must contain 1..64 ASCII letters, digits, underscores or hyphens",
        ));
    }
    Ok(())
}

pub fn pipe_name(name: &str) -> io::Result<String> {
    validate_name(name)?;
    Ok(format!(r"\\.\pipe\wincan_{name}_v1"))
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub id: u32,
    #[serde(default)]
    pub extended: bool,
    #[serde(default)]
    pub rtr: bool,
    #[serde(default)]
    pub error: bool,
    pub dlc: u8,
    #[serde(default)]
    pub data: Vec<u8>,
}

impl Frame {
    pub fn to_can(&self) -> io::Result<CanFrame> {
        let invalid = |message| io::Error::new(io::ErrorKind::InvalidInput, message);
        if self.dlc > 8 || self.data.len() > 8 || (self.rtr && self.error) {
            return Err(invalid(
                "Protocol v1 supports classic CAN frames with DLC <= 8",
            ));
        }
        let result = if self.error {
            if self.extended || self.dlc != 0 || !self.data.is_empty() {
                return Err(invalid("Error frames carry an error mask only"));
            }
            CanFrame::new_error(self.id)
        } else if self.rtr {
            if !self.data.is_empty() {
                return Err(invalid("RTR frames must have empty data"));
            }
            CanFrame::new_remote(self.id, self.dlc as usize, self.extended)
        } else {
            if self.data.len() != self.dlc as usize {
                return Err(invalid("DLC must equal data length"));
            }
            if self.extended {
                CanFrame::new_eff(self.id, &self.data)
            } else {
                CanFrame::new(self.id, &self.data)
            }
        };
        result.map_err(invalid)
    }

    pub fn raw_id(&self) -> u32 {
        self.id
            | if self.extended { 0x8000_0000 } else { 0 }
            | if self.rtr { 0x4000_0000 } else { 0 }
            | if self.error { 0x2000_0000 } else { 0 }
    }
}

impl From<&CanFrame> for Frame {
    fn from(frame: &CanFrame) -> Self {
        Self {
            id: frame.id(),
            extended: frame.is_extended(),
            rtr: frame.is_rtr(),
            error: frame.is_error(),
            dlc: frame.dlc() as u8,
            data: if frame.is_rtr() {
                vec![]
            } else {
                frame.data().to_vec()
            },
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Filter {
    pub id: u32,
    pub mask: u32,
    #[serde(default)]
    pub invert: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Subscription {
    pub receive: bool,
    pub queue_capacity: usize,
    pub filters: Vec<Filter>,
    pub join_filters: bool,
    pub error_mask: u32,
    pub receive_own_messages: bool,
}

impl Default for Subscription {
    fn default() -> Self {
        Self {
            receive: true,
            queue_capacity: 1024,
            filters: vec![],
            join_filters: false,
            error_mask: 0,
            receive_own_messages: false,
        }
    }
}

impl Subscription {
    pub fn validate(&self) -> io::Result<()> {
        if !(1..=MAX_QUEUE).contains(&self.queue_capacity)
            || self.filters.len() > MAX_FILTERS
            || self.error_mask > 0x1fff_ffff
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid queue capacity, filter count or error mask",
            ));
        }
        Ok(())
    }
    pub fn matches(&self, frame: &Frame) -> bool {
        if !self.receive {
            return false;
        }
        if frame.error {
            return frame.id & self.error_mask != 0;
        }
        let check = |filter: &Filter| {
            ((frame.raw_id() & filter.mask) == (filter.id & filter.mask)) != filter.invert
        };
        self.filters.is_empty()
            || if self.join_filters {
                self.filters.iter().all(check)
            } else {
                self.filters.iter().any(check)
            }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub version: u16,
    pub id: u64,
    #[serde(flatten)]
    pub command: Command,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Command {
    Hello {
        #[serde(default)]
        subscription: Subscription,
    },
    Subscribe {
        subscription: Subscription,
    },
    Send {
        frame: Frame,
    },
    Status,
    Clients,
    SetUp {
        up: bool,
    },
    Configure {
        #[serde(default)]
        bitrate: Option<u32>,
        #[serde(default)]
        timestamps: Option<bool>,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InterfaceState {
    Down,
    Up,
    Disconnected,
    Faulted,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BusState {
    #[default]
    Unknown,
    ErrorActive,
    ErrorWarning,
    ErrorPassive,
    BusOff,
    Stopped,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Capabilities {
    /// None means the backend cannot determine the hardware capability.
    pub hardware_can_fd: Option<bool>,
    /// End-to-end FD is not implemented by v1.
    pub can_fd: bool,
    pub hardware_timestamps: Option<bool>,
    pub bus_state: bool,
    pub software_tx_echo: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Counters {
    pub rx_frames: u64,
    pub tx_frames: u64,
    pub rx_errors: u64,
    pub tx_errors: u64,
    pub client_drops: u64,
    pub disconnects: u64,
    pub bus_off_events: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClientStats {
    pub client_id: u64,
    pub queue_capacity: usize,
    pub queued: usize,
    pub delivered: u64,
    pub dropped: u64,
    pub filtered: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Status {
    pub name: String,
    pub driver: String,
    pub device: String,
    pub bitrate: u32,
    pub desired_up: bool,
    pub state: InterfaceState,
    pub bus_state: BusState,
    pub timestamps: bool,
    pub capabilities: Capabilities,
    pub counters: Counters,
    pub active_clients: usize,
    pub last_error: Option<String>,
    pub last_rx_timestamp_us: Option<u64>,
    pub hardware_rx_error_counter: Option<u32>,
    pub hardware_tx_error_counter: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum Reply {
    Hello { client_id: u64, status: Status },
    Ok,
    Status { status: Status },
    Clients { clients: Vec<ClientStats> },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Rx,
    LocalTx,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FrameEvent {
    pub sequence: u64,
    pub frame: Frame,
    pub direction: Direction,
    pub source_client_id: Option<u64>,
    /// Host UNIX time, in microseconds. Local TX time is queue admission.
    pub timestamp_us: u64,
    /// Backend-specific clock in microseconds, never UNIX time.
    pub hardware_timestamp_us: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerMessage {
    pub version: u16,
    #[serde(flatten)]
    pub body: ServerBody,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ServerBody {
    Reply {
        id: u64,
        reply: Reply,
    },
    Error {
        id: u64,
        code: String,
        message: String,
    },
    Frame {
        event: FrameEvent,
    },
}

pub async fn read_message<R: AsyncRead + Unpin, T: serde::de::DeserializeOwned>(
    reader: &mut R,
) -> io::Result<T> {
    let len = reader.read_u32_le().await? as usize;
    if len == 0 || len > MAX_MESSAGE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid protocol message length",
        ));
    }
    let mut bytes = vec![0; len];
    reader.read_exact(&mut bytes).await?;
    serde_json::from_slice(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

pub async fn write_message<W: AsyncWrite + Unpin, T: Serialize>(
    writer: &mut W,
    value: &T,
) -> io::Result<()> {
    let bytes = serde_json::to_vec(value).map_err(io::Error::other)?;
    if bytes.is_empty() || bytes.len() > MAX_MESSAGE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Protocol message too large",
        ));
    }
    writer.write_u32_le(bytes.len() as u32).await?;
    writer.write_all(&bytes).await
}
