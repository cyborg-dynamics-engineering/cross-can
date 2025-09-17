use socketcan::{self, tokio::CanSocket};

impl CrossCanSocket for CanSocket {
    fn open(interface: &str) -> std::io::Result<Self> {
        let sock = CanSocket::open(interface)?;
        Ok(Self { sock })
    }

    async fn read_frame(&mut self) -> std::io::Result<CanFrame> {
        match self.sock.read_frame().await {
            Ok(frame) => Ok(frame.into()),
            Err(e) => Err(e),
        }
    }

    async fn write_frame(&mut self, frame: CanFrame) -> std::io::Result<()> {
        self.sock.write_frame(frame.into()).await
    }
}
