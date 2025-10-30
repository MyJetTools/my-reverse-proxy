use std::{sync::Arc, time::Duration};

use my_ssh::SshCredentials;
use rust_extensions::{remote_endpoint::RemoteEndpointOwned, UnsafeValue};
use tokio::sync::Mutex;

use crate::{app::APP_CTX, h1_proxy_server::H1Reader, network_stream::*, tcp_utils::LoopBuffer};

pub struct H1RemoteConnectionReadPart<
    TNetworkReadPart: NetworkStreamReadPart + Send + Sync + 'static,
> {
    pub h1_reader: Mutex<Option<H1Reader<TNetworkReadPart>>>,
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
    write_part: Option<TNetworkWritePart>,
    read_half: Arc<H1RemoteConnectionReadPart<TNetworkReadPart>>,
    ssh_credentials: Option<Arc<SshCredentials>>,
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
        let ssh_session = if let Some(ssh_credentials) = ssh_credentials.as_ref() {
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
            ssh_session,
            server_name,
            remote_endpoint,
            timeout,
        )
        .await?;

        let (read_part, write_half) = result.split();

        let result = Self {
            write_part: Some(write_half),
            read_half: H1RemoteConnectionReadPart {
                h1_reader: Mutex::new(Some(H1Reader::new(read_part))),
                disconnected: false.into(),
            }
            .into(),
            ssh_credentials,
        };

        Ok(result)
    }

    pub fn is_disconnected(&self) -> bool {
        self.read_half.disconnected.get_value()
    }

    pub async fn send_with_timeout(
        &mut self,
        payload: &[u8],
        time_out: Duration,
    ) -> Result<(), NetworkError> {
        if let Some(write_part) = self.write_part.as_mut() {
            return write_part.write_all_with_timeout(payload, time_out).await;
        }

        Err(NetworkError::Disconnected)
    }

    pub async fn write_to_socket(&mut self, payload: &[u8]) -> Result<(), std::io::Error> {
        if let Some(write_part) = self.write_part.as_mut() {
            return write_part.write_to_socket(payload).await;
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionAborted,
            "Disconnected",
        ))
    }

    pub async fn shutdown_socket(&mut self) {
        if let Some(write_part) = self.write_part.as_mut() {
            write_part.shutdown_socket().await;
        }
    }

    pub fn get_read_part(&self) -> Arc<H1RemoteConnectionReadPart<TNetworkReadPart>> {
        self.read_half.clone()
    }

    pub async fn get_read_and_write_parts(
        &mut self,
    ) -> (TNetworkReadPart, TNetworkWritePart, LoopBuffer) {
        let mut write_access = self.read_half.h1_reader.lock().await;
        let h1_reader = write_access.take().unwrap();
        let (read_part, loop_buffer) = h1_reader.into_read_part();

        (read_part, self.write_part.take().unwrap(), loop_buffer)
    }
}

impl<
        TNetworkWritePart: NetworkStreamWritePart + Send + Sync + 'static,
        TNetworkReadPart: NetworkStreamReadPart + Send + Sync + 'static,
    > Drop for Http1ConnectionInner<TNetworkWritePart, TNetworkReadPart>
{
    fn drop(&mut self) {
        if let Some(ssh_credentials) = self.ssh_credentials.take() {
            tokio::spawn(async move {
                APP_CTX
                    .ssh_sessions_pool
                    .connection_is_dropped(&ssh_credentials)
                    .await;
            });
        }
    }
}
