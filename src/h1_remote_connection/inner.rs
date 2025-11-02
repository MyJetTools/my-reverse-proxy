use std::{
    sync::{atomic::AtomicU64, Arc},
    time::Duration,
};

use my_ssh::SshCredentials;
use rust_extensions::{remote_endpoint::RemoteEndpointOwned, UnsafeValue};

use crate::{app::SshSessionHandler, network_stream::*};

lazy_static::lazy_static!(
       pub static ref CONN_ID: NextConnectionId = {
           NextConnectionId::new()
    };
);

pub struct NextConnectionId(AtomicU64);

impl NextConnectionId {
    pub fn new() -> Self {
        Self(AtomicU64::new(0))
    }

    pub fn get_next(&self) -> u64 {
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }
}

pub struct Http1ConnectionInner<TNetworkWritePart: NetworkStreamWritePart + Send + Sync + 'static> {
    pub remote_write_part: TNetworkWritePart,
    disconnected: Arc<UnsafeValue<bool>>,
}

impl<TNetworkWritePart: NetworkStreamWritePart + Send + Sync + 'static>
    Http1ConnectionInner<TNetworkWritePart>
{
    pub async fn connect<
        TNetworkReadPart: NetworkStreamReadPart + Send + Sync + 'static,
        TNetworkStream: NetworkStream<WritePart = TNetworkWritePart, ReadPart = TNetworkReadPart>
            + Send
            + Sync
            + 'static,
    >(
        gateway_id: Option<&Arc<String>>,
        ssh_credentials: Option<Arc<SshCredentials>>,
        server_name: Option<&str>,
        remote_endpoint: &Arc<RemoteEndpointOwned>,
        timeout: Duration,
    ) -> Result<(Self, TNetworkReadPart, Option<SshSessionHandler>), NetworkError> {
        let ssh_session_handler = if let Some(ssh_credentials) = ssh_credentials.as_ref() {
            let ssh_session = crate::app::APP_CTX
                .ssh_sessions_pool
                .get(ssh_credentials)
                .await;
            Some(ssh_session)
        } else {
            None
        };

        let result = TNetworkStream::connect(
            gateway_id,
            ssh_session_handler
                .as_ref()
                .map(|itm| itm.ssh_session.clone()),
            server_name,
            remote_endpoint,
            timeout,
        )
        .await?;

        let (read_part, write_part) = result.split();

        let result = Self {
            remote_write_part: write_part,
            disconnected: Arc::new(false.into()),
        };

        Ok((result, read_part, ssh_session_handler))
    }

    pub fn get_remote_disconnect_trigger(&self) -> Arc<UnsafeValue<bool>> {
        self.disconnected.clone()
    }

    pub fn is_disconnected(&self) -> bool {
        self.disconnected.get_value()
    }

    pub async fn send_with_timeout(
        &mut self,
        payload: &[u8],
        time_out: Duration,
    ) -> Result<(), NetworkError> {
        return self
            .remote_write_part
            .write_all_with_timeout(payload, time_out)
            .await;
    }

    pub async fn flush_it(&mut self) -> Result<(), NetworkError> {
        self.remote_write_part.flush_it().await
    }

    pub async fn write_to_socket(&mut self, payload: &[u8]) -> Result<(), std::io::Error> {
        return self.remote_write_part.write_to_socket(payload).await;
    }

    pub async fn shutdown_socket(&mut self) {
        self.remote_write_part.shutdown_socket().await
    }
}
