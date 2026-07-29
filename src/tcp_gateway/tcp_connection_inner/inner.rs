use std::sync::Arc;

use encryption::aes::AesKey;
use rust_extensions::UnsafeValue;
use tokio::sync::mpsc;

use crate::network_stream::{MyOwnedWriteHalf, NetworkError};

const WRITE_CHANNEL_CAPACITY: usize = 1024;

/// Channel-backed writer wrapper used by both gateway peer and forwarded
/// downstream connections.
///
/// In **gateway peer** mode the writer treats `send_payload` items as
/// plaintext inner frames (`[u32 LEN][u8 TYPE][PAYLOAD]`) and on each flush
/// either encrypts each one individually (no compression) or wraps the
/// whole accumulator into a `COMPRESSED_BATCH` frame and encrypts that.
///
/// In **raw passthrough** mode the writer just concatenates whatever bytes
/// it receives and writes them to the socket unmodified — used for the
/// forwarded TCP stream toward the downstream service.
pub struct TcpConnectionInner {
    sender: parking_lot::Mutex<Option<mpsc::Sender<Vec<u8>>>>,
    is_connected: Box<UnsafeValue<bool>>,
    aes_key_opt: Option<Arc<AesKey>>,
}

impl TcpConnectionInner {
    pub fn new_gateway_peer(
        write_half: MyOwnedWriteHalf,
        aes_key: Arc<AesKey>,
        compress_outbound: bool,
    ) -> Arc<Self> {
        let (sender, receiver) = mpsc::channel::<Vec<u8>>(WRITE_CHANNEL_CAPACITY);
        let result = Arc::new(Self {
            sender: parking_lot::Mutex::new(Some(sender)),
            is_connected: Box::new(true.into()),
            aes_key_opt: Some(aes_key.clone()),
        });

        crate::app::spawn_named(
            "tcp_gateway_write_loop_encrypted",
            crate::tcp_gateway::session::gateway_write_loop(
                write_half,
                receiver,
                aes_key,
                compress_outbound,
            ),
        );

        result
    }

    pub fn new_raw_passthrough(write_half: MyOwnedWriteHalf) -> Arc<Self> {
        let (sender, receiver) = mpsc::channel::<Vec<u8>>(WRITE_CHANNEL_CAPACITY);
        let result = Arc::new(Self {
            sender: parking_lot::Mutex::new(Some(sender)),
            is_connected: Box::new(true.into()),
            aes_key_opt: None,
        });

        crate::app::spawn_named(
            "tcp_gateway_write_loop_raw",
            crate::tcp_gateway::session::raw_write_loop(write_half, receiver),
        );

        result
    }

    /// Returns the gateway peer's AES key. Panics if invoked on a raw
    /// passthrough writer (which is a programming error: only gateway peer
    /// inners hold a key).
    pub fn aes_key(&self) -> &Arc<AesKey> {
        self.aes_key_opt
            .as_ref()
            .expect("aes_key() invoked on raw passthrough writer")
    }

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

    pub async fn disconnect(&self) -> bool {
        let was_connected = self.is_connected.get_value();
        self.is_connected.set_value(false);
        self.sender.lock().take();
        was_connected
    }

    pub async fn flush(&self) -> Result<(), NetworkError> {
        Ok(())
    }
}
