use std::{sync::Arc, time::Duration};

use my_ssh::{SshCredentials, SshSession};
use rust_extensions::remote_endpoint::RemoteEndpointOwned;

use crate::network_stream::*;

pub struct Http1Connection<
    TNetworkWritePart: NetworkStreamWritePart + Send + Sync + 'static,
    TNetworkReadPart: NetworkStreamReadPart + Send + Sync + 'static,
> {
    write_half: TNetworkWritePart,
    read_half: Option<TNetworkReadPart>,
    _ssh_session: Option<Arc<SshSession>>,
}

impl<
        TNetworkWritePart: NetworkStreamWritePart + Send + Sync + 'static,
        TNetworkReadPart: NetworkStreamReadPart + Send + Sync + 'static,
    > Http1Connection<TNetworkWritePart, TNetworkReadPart>
{
    pub async fn connect<
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
    ) -> Result<Self, String> {
        let _ssh_session = ssh_credentials.map(|itm| Arc::new(SshSession::new(itm)));

        let result = TNetworkStream::connect(
            gateway_id,
            _ssh_session.clone(),
            server_name,
            remote_endpoint,
            timeout,
        )
        .await?;

        let (read_half, write_half) = result.split();

        let result = Self {
            write_half,
            read_half: Some(read_half),
            _ssh_session,
        };

        Ok(result)
    }

    pub async fn send(&mut self, payload: &[u8], time_out: Duration) {
        self.write_half
            .write_all_with_timeout(payload, time_out)
            .await
            .unwrap()
    }

    pub async fn write_to_socket(&mut self, payload: &[u8]) -> Result<(), std::io::Error> {
        self.write_half.write_to_socket(payload).await
    }

    pub async fn shutdown_socket(&mut self) {
        self.write_half.shutdown_socket().await;
    }

    pub fn get_read_part(&mut self) -> TNetworkReadPart {
        self.read_half.take().unwrap()
    }
}
