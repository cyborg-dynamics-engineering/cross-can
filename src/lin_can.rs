///
/// lin_can.rs
///
/// Implementation of CanInterface for Linux using SocketCan.
///
use crate::{CanInterface, can::CanFrame};
use socketcan::{nl, tokio::CanSocket};

pub struct LinuxCan {
    socket: CanSocket,
    interface: String,
}

impl CanInterface for LinuxCan {
    fn open(interface: &str) -> std::io::Result<Self> {
        Ok(LinuxCan {
            socket: CanSocket::open(interface)?,
            interface: interface.to_string(),
        })
    }

    async fn read_frame(&mut self) -> std::io::Result<CanFrame> {
        match self.socket.read_frame().await {
            Ok(frame) => Ok(frame.into()),
            Err(e) => Err(e),
        }
    }

    async fn write_frame(&mut self, frame: CanFrame) -> std::io::Result<()> {
        self.socket.write_frame(frame.into()).await
    }

    async fn get_bitrate(&mut self) -> std::io::Result<Option<u32>> {
        let iface = nl::CanInterface::open(&self.interface)?;

        iface
            .bit_rate()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    }
}
