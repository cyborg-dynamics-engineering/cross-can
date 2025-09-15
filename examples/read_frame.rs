use crosscan::CrossCanSocket;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let interface = std::env::args().nth(1).expect("Usage: program <interface>");

    // Open the CrossCanSocket (async, works on both Unix and Windows)
    let mut socket = CrossCanSocket::open(&interface)?;

    println!("Listening on CAN interface: {}", interface);

    // Loop to read and print incoming CAN frames
    loop {
        let frame = socket.read_frame().await?;
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
