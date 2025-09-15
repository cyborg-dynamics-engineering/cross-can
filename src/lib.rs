pub mod can;
use can::CanFrame;

#[cfg(target_os = "macos")]
compile_error!("Currently only linux or windows are supported");

#[cfg(target_os = "linux")]
use socketcan::{self, tokio::CanSocket};

#[cfg(target_os = "windows")]
pub mod win_can;
#[cfg(target_os = "windows")]
use win_can::CanSocket;

pub struct CrossCanSocket {
    sock: CanSocket,
}

impl CrossCanSocket {
    /// Open method for both platforms
    pub fn open(interface: &str) -> std::io::Result<Self> {
        let sock = CanSocket::open(interface)?;
        Ok(Self { sock })
    }

    /// Async read frame unified for both platforms
    pub async fn read_frame(&mut self) -> std::io::Result<CanFrame> {
        match self.sock.read_frame().await {
            Ok(frame) => Ok(frame.into()),
            Err(e) => Err(e),
        }
    }

    /// Async write frame unified for both platforms
    pub async fn write_frame(&mut self, frame: CanFrame) -> std::io::Result<()> {
        self.sock.write_frame(frame.into()).await
    }
}
