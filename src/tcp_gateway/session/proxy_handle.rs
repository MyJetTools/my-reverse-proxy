use std::{sync::Arc, time::Duration};

use rust_extensions::SliceOrVec;
use tokio::sync::{mpsc, oneshot};

use crate::tcp_gateway::TcpGatewayContract;

use super::session_struct::Session;

const DEFAULT_DATA_CHANNEL_CAPACITY: usize = 256;

/// Phase 2 placeholder for `ProxyConnection`.
///
/// Ties together a `cid` with its `Arc<Session>` and the per-slot `data_rx`
/// that receives `BackwardPayload` bytes from the gateway. Phase 3 will wrap
/// this in `AsyncRead + AsyncWrite` so Hyper can use it as a transparent
/// socket.
pub struct ProxyHandle {
    pub cid: u32,
    pub session: Arc<Session>,
    pub data_rx: mpsc::Receiver<Vec<u8>>,
}

impl ProxyHandle {
    /// Send a chunk of bytes downstream (peer side will write them to its real
    /// downstream socket). TCP-like backpressure: returns `Err` if the gateway
    /// session has died.
    pub async fn send(&self, payload: Vec<u8>) -> Result<(), String> {
        let frame = TcpGatewayContract::ForwardPayload {
            connection_id: self.cid,
            payload: SliceOrVec::AsVec(payload),
        }
        .to_vec(&self.session.aes_key, self.session.support_compression);

        self.session
            .write_tx
            .send(frame)
            .await
            .map_err(|_| "gateway write_tx closed".to_string())
    }

    /// Pull the next chunk of bytes coming from the peer's downstream socket.
    /// Returns `None` when the peer has closed this cid or the gateway session
    /// has died.
    pub async fn recv(&mut self) -> Option<Vec<u8>> {
        self.data_rx.recv().await
    }
}

impl Drop for ProxyHandle {
    fn drop(&mut self) {
        // Best-effort: remove the slot so any further BackwardPayload from peer
        // is silently dropped on this end. The peer side gets a proper close
        // signal via `Session::send_connection_error_blocking_on_drop` only if
        // the caller invoked the explicit `close` flow — Phase 3 will wire
        // that into `AsyncWrite::poll_shutdown`.
        self.session.remove_slot(self.cid);
    }
}

impl Session {
    /// Initiate a forwarded connection to `host` via this gateway session.
    ///
    /// Allocates a new `cid`, registers the per-slot inbound channel and the
    /// `pending` oneshot, sends a `Connect` frame through the gateway, and
    /// awaits the peer's `Connected` (or `ConnectionError`) reply within the
    /// given timeout.
    pub async fn connect_to_remote(
        self: &Arc<Self>,
        host: String,
        connect_timeout: Duration,
    ) -> Result<ProxyHandle, String> {
        let cid = self.next_cid();

        // Pre-register the slot so any racy BackwardPayload that arrives just
        // after `Connected` is not dropped.
        let (data_tx, data_rx) = mpsc::channel::<Vec<u8>>(DEFAULT_DATA_CHANNEL_CAPACITY);
        self.insert_slot(cid, data_tx);

        let (reply_tx, reply_rx) = oneshot::channel();
        self.insert_pending(cid, reply_tx);

        let frame = TcpGatewayContract::Connect {
            connection_id: cid,
            timeout: connect_timeout,
            remote_host: &host,
        }
        .to_vec(&self.aes_key, false);

        if self.write_tx.send(frame).await.is_err() {
            self.take_pending(cid);
            self.remove_slot(cid);
            return Err("gateway write_tx closed".into());
        }

        // Bound the wait — the peer should reply within the connect_timeout
        // (peer may also internally apply its own timeout).
        let outcome = match tokio::time::timeout(
            connect_timeout + Duration::from_secs(1),
            reply_rx,
        )
        .await
        {
            Ok(Ok(res)) => res,
            Ok(Err(_)) => Err("gateway disconnected".to_string()),
            Err(_) => {
                self.take_pending(cid);
                Err("connect_to_remote: oneshot timeout".to_string())
            }
        };

        match outcome {
            Ok(()) => Ok(ProxyHandle {
                cid,
                session: self.clone(),
                data_rx,
            }),
            Err(err) => {
                self.remove_slot(cid);
                Err(err)
            }
        }
    }
}
