# Cross CAN

Provides a cross-platform CAN socket for interacting with open CAN connections in rust.


## Windows interface service client

The `protocol` module defines the portable Windows CAN service v1 wire format.
On Windows, `service::ServiceClient` connects to a running `win-can-utils`
interface broker with independent receive queues, filters, transmit echo,
status, and configuration controls. The existing async `CanInterface` and
legacy `WindowsCan` APIs are retained.

```rust,no_run
use crosscan::{protocol::{Command, Subscription}, service::ServiceClient};

# async fn example() -> std::io::Result<()> {
let mut bus = ServiceClient::connect("can0", Subscription::default()).await?;
let control = bus.control();
let status = control.request(Command::Status).await?;
println!("{status:?}");
let event = bus.next_event().await?;
println!("{event:?}; local drops={}", bus.local_drops());
# Ok(()) }
```

See the [protocol and lifecycle guide](docs/client-protocol-v1.md) and the
[Python example](examples/service_client.py). Server integration tests live in
the companion [win-can-utils repository](https://github.com/Cyborg-Dynamics-Engineering/win-can-utils).

## License
Cyder Stream is licensed under either of

    Apache License, Version 2.0 (LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
    MIT license (LICENSE-MIT or http://opensource.org/licenses/MIT)

at your option.


## Contribution
Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
