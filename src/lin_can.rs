///
/// lin_can.rs
///
/// Implementation of CanInterface for Linux using SocketCan.
///
use crate::{CanInterface, can::CanFrame};
use socketcan::tokio::CanSocket;

pub struct LinuxCan {
    socket: CanSocket,
}

impl CanInterface for LinuxCan {
    fn open(interface: &str) -> std::io::Result<Self> {
        Ok(LinuxCan {
            socket: CanSocket::open(interface)?,
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
}
