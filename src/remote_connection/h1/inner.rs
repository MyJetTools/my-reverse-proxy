use std::{sync::Arc, time::Duration};

use my_ssh::{SshCredentials, SshSession};
use rust_extensions::{remote_endpoint::RemoteEndpointOwned, UnsafeValue};
use tokio::sync::Mutex;

use crate::{h1_proxy_server::H1ReadPart, network_stream::*};

pub struct H1RemoteConnectionReadPart<
    TNetworkReadPart: NetworkStreamReadPart + Send + Sync + 'static,
> {
    pub read_half: Mutex<H1ReadPart<TNetworkReadPart>>,
    disconnected: UnsafeValue<bool>,
}

impl<TNetworkReadPart: NetworkStreamReadPart + Send + Sync + 'static>
    H1RemoteConnectionReadPart<TNetworkReadPart>
{
    pub fn get_disconnected(&self) -> bool {
        self.disconnected.get_value()
    }

    pub fn set_disconnected(&self) {
        self.disconnected.set_value(true);
    }
}

pub struct Http1ConnectionInner<
    TNetworkWritePart: NetworkStreamWritePart + Send + Sync + 'static,
    TNetworkReadPart: NetworkStreamReadPart + Send + Sync + 'static,
> {
    write_half: TNetworkWritePart,
    read_half: Arc<H1RemoteConnectionReadPart<TNetworkReadPart>>,
    _ssh_session: Option<Arc<SshSession>>,
}

impl<
        TNetworkWritePart: NetworkStreamWritePart + Send + Sync + 'static,
        TNetworkReadPart: NetworkStreamReadPart + Send + Sync + 'static,
    > Http1ConnectionInner<TNetworkWritePart, TNetworkReadPart>
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

        let (read_part, write_half) = result.split();

        let result = Self {
            write_half,
            read_half: H1RemoteConnectionReadPart {
                read_half: Mutex::new(H1ReadPart::new(read_part)),
                disconnected: false.into(),
            }
            .into(),
            _ssh_session,
        };

        Ok(result)
    }

    pub fn is_disconnected(&self) -> bool {
        self.read_half.disconnected.get_value()
    }

    pub async fn send_with_timeout(&mut self, payload: &[u8], time_out: Duration) {
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

    pub fn get_read_part(&self) -> Arc<H1RemoteConnectionReadPart<TNetworkReadPart>> {
        self.read_half.clone()
    }
}
