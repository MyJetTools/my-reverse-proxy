use std::sync::Arc;

use encryption::aes::AesKey;
use rust_extensions::UnsafeValue;
use tokio::sync::mpsc;

use crate::network_stream::{MyOwnedWriteHalf, NetworkError};

const WRITE_CHANNEL_CAPACITY: usize = 1024;

/// Bug fix for the long-running gateway hang.
///
/// Previous implementation used a `SendBuffer` (under `parking_lot::Mutex`) +
/// a signal-channel `mpsc::Sender<()>` to wake the writer task. This had a
/// lost-wakeup race: producer pushes into `SendBuffer`, then `try_send(())`
/// could fail silently when the signal channel was momentarily full while the
/// writer was just returning to `recv().await`. The data stayed in the buffer
/// but no one woke the writer ⇒ permanent hang.
///
/// New implementation uses a single bounded `mpsc::Sender<Vec<u8>>` that
/// carries **the payload itself**, not a notification. Lost-wakeup is
/// impossible by construction: every payload that enters the channel gets
/// delivered or returns an explicit `Err` to the producer.
pub struct TcpConnectionInner {
    /// `None` once `disconnect()` has run — drops all senders so the writer
    /// task observes channel close and exits cleanly.
    sender: parking_lot::Mutex<Option<mpsc::Sender<Vec<u8>>>>,
    is_connected: Box<UnsafeValue<bool>>,
    pub aes_key: Arc<AesKey>,
}

impl TcpConnectionInner {
    /// Construct the inner and immediately spawn its write loop. The caller
    /// only ever needs the `Arc<Self>` — the channel is fully encapsulated.
    pub fn new(write_half: MyOwnedWriteHalf, aes_key: Arc<AesKey>) -> Arc<Self> {
        let (sender, receiver) = mpsc::channel::<Vec<u8>>(WRITE_CHANNEL_CAPACITY);
        let result = Arc::new(Self {
            sender: parking_lot::Mutex::new(Some(sender)),
            is_connected: Box::new(true.into()),
            aes_key,
        });

        // Reuse the new race-free writer from `session::gateway_write_loop`.
        tokio::spawn(crate::tcp_gateway::session::gateway_write_loop(
            write_half, receiver,
        ));

        result
    }

    /// Best-effort enqueue. **Takes ownership** of the encoded frame so the
    /// caller's `Vec<u8>` (typically the result of `TcpGatewayContract::to_vec`)
    /// is moved into the write channel without an extra copy.
    ///
    /// Returns `false` when the connection is already disconnected or the
    /// write channel is at capacity. In those cases the `payload` is dropped
    /// — equivalent to the old behavior of silent loss along that path.
    pub fn send_payload(&self, payload: Vec<u8>) -> bool {
        if !self.is_connected.get_value() {
            return false;
        }
        let guard = self.sender.lock();
        let Some(sender) = guard.as_ref() else {
            return false;
        };
        sender.try_send(payload).is_ok()
    }

    pub fn is_connected(&self) -> bool {
        self.is_connected.get_value()
    }

    /// Drop the writer's sender — the bounded channel closes, the writer task
    /// drains any in-flight bytes, calls `shutdown_socket`, and exits.
    pub async fn disconnect(&self) -> bool {
        let was_connected = self.is_connected.get_value();
        self.is_connected.set_value(false);
        self.sender.lock().take();
        was_connected
    }

    /// No-op: the underlying write half is owned by the writer task and cannot
    /// be flushed from outside without sending an explicit `Flush` sentinel
    /// through the channel. For TCP this is harmless — `AsyncWriteExt::flush`
    /// on `tokio::net::tcp::OwnedWriteHalf` is itself a no-op. If a future
    /// flush-ack signal becomes necessary, it can be added without changing
    /// this signature.
    pub async fn flush(&self) -> Result<(), NetworkError> {
        Ok(())
    }
}
