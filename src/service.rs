//! Async, duplex v1 client. Legacy `WindowsCan` remains available separately.
use crate::protocol::*;
use std::{
    collections::HashMap,
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    net::windows::named_pipe::ClientOptions,
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

type Pending = oneshot::Sender<io::Result<Reply>>;
type Outgoing = (Command, Pending);

#[derive(Clone)]
pub struct ServiceControl {
    tx: mpsc::Sender<Outgoing>,
}

impl ServiceControl {
    /// Requests have a 15-second response deadline. A timeout does not undo a command.
    pub async fn request(&self, command: Command) -> io::Result<Reply> {
        let (tx, rx) = oneshot::channel();
        tokio::time::timeout(Duration::from_secs(15), async {
            self.tx
                .send((command, tx))
                .await
                .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "Service disconnected"))?;
            rx.await
                .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "Service disconnected"))?
        })
        .await
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::TimedOut,
                "Service response deadline exceeded",
            )
        })?
    }
}

pub struct ServiceClient {
    control: ServiceControl,
    events: mpsc::Receiver<FrameEvent>,
    local_drops: Arc<AtomicU64>,
    task: JoinHandle<()>,
    pub client_id: u64,
}

impl ServiceClient {
    pub async fn connect(name: &str, subscription: Subscription) -> io::Result<Self> {
        subscription.validate()?;
        let path = pipe_name(name)?;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        let pipe = loop {
            match ClientOptions::new().open(&path) {
                Ok(pipe) => break pipe,
                Err(error)
                    if matches!(error.raw_os_error(), Some(2 | 231))
                        && tokio::time::Instant::now() < deadline =>
                {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(error) => return Err(error),
            }
        };
        let (mut reader, mut writer) = tokio::io::split(pipe);
        let (tx, mut rx) = mpsc::channel::<Outgoing>(32);
        let (events_tx, events) = mpsc::channel(subscription.queue_capacity);
        let local_drops = Arc::new(AtomicU64::new(0));
        let drops = local_drops.clone();
        let task = tokio::spawn(async move {
            let pending = Mutex::new(HashMap::<u64, Pending>::new());
            let write = async {
                let mut id = 0u64;
                while let Some((command, response)) = rx.recv().await {
                    {
                        let mut pending = pending.lock().unwrap();
                        pending.retain(|_, response| !response.is_closed());
                        if pending.len() >= 128 {
                            let _ = response.send(Err(io::Error::new(
                                io::ErrorKind::WouldBlock,
                                "Too many outstanding service requests",
                            )));
                            continue;
                        }
                    }
                    id = id
                        .checked_add(1)
                        .ok_or_else(|| io::Error::other("Request ID exhausted"))?;
                    pending.lock().unwrap().insert(id, response);
                    write_message(
                        &mut writer,
                        &Request {
                            version: VERSION,
                            id,
                            command,
                        },
                    )
                    .await?;
                }
                Ok::<_, io::Error>(())
            };
            let read = async {
                loop {
                    let message: ServerMessage = read_message(&mut reader).await?;
                    if message.version != VERSION {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "Unsupported server version",
                        ));
                    }
                    match message.body {
                        ServerBody::Frame { event } => {
                            if let Err(mpsc::error::TrySendError::Full(_)) =
                                events_tx.try_send(event)
                            {
                                drops.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        ServerBody::Reply { id, reply } => {
                            if let Some(response) = pending.lock().unwrap().remove(&id) {
                                let _ = response.send(Ok(reply));
                            }
                        }
                        ServerBody::Error { id, code, message } => {
                            let kind = match code.as_str() {
                                "interface_down" => io::ErrorKind::NotConnected,
                                "invalid_request" | "invalid_version" => {
                                    io::ErrorKind::InvalidInput
                                }
                                _ => io::ErrorKind::Other,
                            };
                            if let Some(response) = pending.lock().unwrap().remove(&id) {
                                let _ = response
                                    .send(Err(io::Error::new(kind, format!("{code}: {message}"))));
                            }
                        }
                    }
                }
                #[allow(unreachable_code)]
                Ok::<_, io::Error>(())
            };
            let result = tokio::select! { result = write => result, result = read => result };
            let message = result
                .err()
                .map(|e| e.to_string())
                .unwrap_or_else(|| "Service disconnected".into());
            for (_, response) in pending.into_inner().unwrap() {
                let _ = response.send(Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    message.clone(),
                )));
            }
        });
        let mut client = Self {
            control: ServiceControl { tx },
            events,
            local_drops,
            task,
            client_id: 0,
        };
        match client.request(Command::Hello { subscription }).await? {
            Reply::Hello { client_id, .. } => client.client_id = client_id,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Expected hello reply",
                ));
            }
        }
        Ok(client)
    }

    pub fn control(&self) -> ServiceControl {
        self.control.clone()
    }
    pub async fn request(&self, command: Command) -> io::Result<Reply> {
        self.control.request(command).await
    }
    pub async fn next_event(&mut self) -> io::Result<FrameEvent> {
        self.events
            .recv()
            .await
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "Service disconnected"))
    }
    /// Frames discarded by this client's own bounded event queue (in addition to server drops).
    pub fn local_drops(&self) -> u64 {
        self.local_drops.load(Ordering::Relaxed)
    }
}

impl Drop for ServiceClient {
    fn drop(&mut self) {
        self.task.abort();
    }
}
