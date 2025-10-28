pub mod can;
use can::CanFrame;

/// A generic async CAN interface for reading and writing CAN frames
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

    /// Returns the bitrate of the CAN bus. Returns None if there is no active connection
    fn get_bitrate(&mut self) -> std::io::Result<Option<u32>>;
}

#[cfg(target_os = "macos")]
compile_error!("Currently only linux or windows are supported");

#[cfg(target_os = "linux")]
pub mod lin_can;

#[cfg(target_os = "windows")]
pub mod win_can;
