use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use http_body_util::{combinators::BoxBody, Full};
use hyper::client::conn::http1::SendRequest;
use rust_extensions::date_time::DateTimeAsMicroseconds;
use tokio::sync::Mutex;

use crate::{hyper::*, MyHttpClientError};

pub enum MyHttpHyperConnectionState {
    Disconnected,

    Connected {
        current_connection_id: u64,
        connected: DateTimeAsMicroseconds,
        send_request: SendRequest<Full<Bytes>>,
        upgraded_to_websocket: bool,
    },

    Disposed,
}

impl MyHttpHyperConnectionState {
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }
}

pub struct MyHttpHyperClientInner {
    pub state: Mutex<MyHttpHyperConnectionState>,
    pub name: Arc<String>,
    pub metrics: Option<std::sync::Arc<dyn MyHttpHyperClientMetrics + Send + Sync + 'static>>,
}

impl MyHttpHyperClientInner {
    pub fn new(
        name: Arc<String>,
        metrics: Option<std::sync::Arc<dyn MyHttpHyperClientMetrics + Send + Sync + 'static>>,
    ) -> Self {
        Self {
            state: Mutex::new(MyHttpHyperConnectionState::Disconnected),

            name,

            metrics,
        }
    }

    pub async fn send_payload(
        &self,
        req: &hyper::Request<Full<Bytes>>,
        request_timeout: Duration,
    ) -> Result<hyper::Response<BoxBody<Bytes, String>>, SendHyperPayloadError> {
        let (send_request_feature, connected, current_connection_id) = {
            let mut state = self.state.lock().await;
            match &mut *state {
                MyHttpHyperConnectionState::Disconnected => {
                    return Err(SendHyperPayloadError::Disconnected);
                }
                MyHttpHyperConnectionState::Connected {
                    current_connection_id,
                    connected,
                    send_request,
                    upgraded_to_websocket,
                } => {
                    if *upgraded_to_websocket {
                        return Err(SendHyperPayloadError::UpgradedToWebsocket);
                    }
                    (
                        send_request.send_request(req.clone()),
                        *connected,
                        *current_connection_id,
                    )
                }

                MyHttpHyperConnectionState::Disposed => {
                    return Err(SendHyperPayloadError::Disposed);
                }
            }
        };

        let result = tokio::time::timeout(request_timeout, send_request_feature).await;

        if result.is_err() {
            self.disconnect(current_connection_id).await;
            return Err(SendHyperPayloadError::RequestTimeout(request_timeout));
        }

        let result = result.unwrap();

        match result {
            Ok(response) => Ok(crate::utils::from_incoming_body(response)),
            Err(err) => {
                self.disconnect(current_connection_id).await;
                Err(SendHyperPayloadError::HyperError { connected, err })
            }
        }
    }

    pub async fn disconnect(&self, connection_id: u64) {
        let mut state = self.state.lock().await;

        match &*state {
            MyHttpHyperConnectionState::Connected {
                current_connection_id,
                ..
            } => {
                if *current_connection_id != connection_id {
                    return;
                }

                if let Some(metrics) = self.metrics.as_ref() {
                    metrics.disconnected(self.name.as_str());
                }
            }
            MyHttpHyperConnectionState::Disconnected => {
                return;
            }
            MyHttpHyperConnectionState::Disposed => {
                return;
            }
        }

        *state = MyHttpHyperConnectionState::Disconnected;
    }

    pub async fn upgrade_to_websocket(&self) -> Result<(), MyHttpClientError> {
        let mut state = self.state.lock().await;

        match &mut *state {
            MyHttpHyperConnectionState::Disconnected => {
                panic!("Http connection is disconnected");
            }
            MyHttpHyperConnectionState::Connected {
                current_connection_id: _,
                connected: _,
                send_request: _,
                upgraded_to_websocket,
            } => {
                if *upgraded_to_websocket {
                    return Err(MyHttpClientError::UpgradedToWebSocket);
                }

                Ok(())
            }
            MyHttpHyperConnectionState::Disposed => {
                panic!("Attempt to upgrade http client to websocket in disposed state");
            }
        }
    }

    pub async fn dispose(&self) {
        let mut state = self.state.lock().await;

        let disposed = match &*state {
            MyHttpHyperConnectionState::Connected { .. } => true,
            MyHttpHyperConnectionState::Disconnected => false,
            MyHttpHyperConnectionState::Disposed => false,
        };

        if disposed {
            if let Some(metrics) = self.metrics.as_ref() {
                metrics.disconnected(self.name.as_str());
            }
        }

        *state = MyHttpHyperConnectionState::Disposed;
    }

    pub async fn force_disconnect(&self) {
        let mut state = self.state.lock().await;
        *state = MyHttpHyperConnectionState::Disconnected;
    }
}
