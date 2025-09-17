use crosscan::CrossCanSocket;

#[cfg(target_os = "linux")]
use crosscan::lin_can::CanSocket;
#[cfg(target_os = "windows")]
use crosscan::win_can::WinCanSocket as CanSocket;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let interface = std::env::args().nth(1).expect("Usage: program <interface>");

    // Open the desired CrossCanSocket depending on OS
    let mut socket = CanSocket::open(&interface)?;
    println!("Listening on CAN interface: {}", interface);

    loop_read_frame(&mut socket).await?;

    Ok(())
}

async fn loop_read_frame<T: CrossCanSocket>(socket: &mut T) -> std::io::Result<()> {
    loop {
        let frame = socket.read().await?;
        println!(
            "{:?} ID=0x{:X} Extended={} RTR={} Error={} [{}]",
            frame.timestamp().unwrap_or(0),
            frame.id(),
            frame.is_extended(),
            frame.is_rtr(),
            frame.is_error(),
            frame
                .data()
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" "),
        );
    }
}
