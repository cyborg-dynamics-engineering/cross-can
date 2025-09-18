use crosscan::CanInterface;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let interface = std::env::args().nth(1).expect("Usage: program <interface>");

    // Open the desired CanInterface depending on OS
    #[cfg(target_os = "linux")]
    let mut can_interface = crosscan::lin_can::LinuxCan::open(&interface)?;
    #[cfg(target_os = "windows")]
    let mut can_interface = crosscan::win_can::WindowsCan::open(&interface)?;

    println!("Listening on CAN interface: {}", interface);
    loop_read_frame(&mut can_interface).await?;
    Ok(())
}

async fn loop_read_frame<T: CanInterface>(can_interface: &mut T) -> std::io::Result<()> {
    loop {
        let frame = can_interface.read_frame().await?;
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
