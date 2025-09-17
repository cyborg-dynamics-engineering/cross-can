pub mod can;
use can::CanFrame;

pub trait CanInterface: Sized {
    /// Opens a CAN interface
    fn open(interface: &str) -> std::io::Result<Self>;

    /// Read a single CAN frame from the interface
    fn read_frame(&mut self)
    -> impl std::future::Future<Output = std::io::Result<CanFrame>> + Send;

    /// Write a single CAN frame from the interface
    fn write_frame(
        &mut self,
        frame: CanFrame,
    ) -> impl std::future::Future<Output = std::io::Result<()>> + Send;
}

#[cfg(target_os = "macos")]
compile_error!("Currently only linux or windows are supported");

#[cfg(target_os = "linux")]
pub mod lin_can;

#[cfg(target_os = "windows")]
pub mod win_can;
