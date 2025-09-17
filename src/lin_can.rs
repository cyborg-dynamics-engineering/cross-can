pub use socketcan::tokio::CanSocket;

use crate::{CrossCanSocket, can::CanFrame};

impl CrossCanSocket for CanSocket {
    fn open(interface: &str) -> std::io::Result<Self> {
        let sock = CanSocket::open(interface)?;
        Ok(sock)
    }

    async fn read(&mut self) -> std::io::Result<CanFrame> {
        match self.read_frame().await {
            Ok(frame) => Ok(frame.into()),
            Err(e) => Err(e),
        }
    }

    async fn write(&mut self, frame: CanFrame) -> std::io::Result<()> {
        self.write_frame(frame.into()).await
    }
}
