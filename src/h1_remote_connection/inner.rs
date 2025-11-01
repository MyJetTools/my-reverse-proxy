use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64},
        Arc,
    },
    time::Duration,
};

use my_ssh::SshCredentials;
use rust_extensions::{date_time::DateTimeAsMicroseconds, remote_endpoint::RemoteEndpointOwned};
use tokio::sync::Mutex;

use crate::{
    app::SshSessionHandler, h1_proxy_server::H1Reader, network_stream::*, tcp_utils::LoopBuffer,
    types::HttpTimeouts,
};

lazy_static::lazy_static!(
       pub static ref CONN_ID: AtomicU64 = {
            AtomicU64::new(0)
    };
);

pub struct H1RemoteConnectionReadPart<
    TNetworkReadPart: NetworkStreamReadPart + Send + Sync + 'static,
> {
    pub h1_reader: Mutex<Option<H1Reader<TNetworkReadPart>>>,
    disconnected: AtomicBool,
    id: u64,
}

impl<TNetworkReadPart: NetworkStreamReadPart + Send + Sync + 'static>
    H1RemoteConnectionReadPart<TNetworkReadPart>
{
    pub fn get_disconnected(&self) -> bool {
        self.disconnected.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn set_disconnected(&self) {
        println!("Settings disconnected true. Location id: {}", self.id);
        self.disconnected
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

pub struct Http1ConnectionInner<
    TNetworkWritePart: NetworkStreamWritePart + Send + Sync + 'static,
    TNetworkReadPart: NetworkStreamReadPart + Send + Sync + 'static,
> {
    write_part: TNetworkWritePart,
    read_half: Arc<H1RemoteConnectionReadPart<TNetworkReadPart>>,
    pub ssh_session_handler: Option<SshSessionHandler>,
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
    ) -> Result<Self, NetworkError> {
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

        println!("Connected to {}", remote_endpoint.get_host_port().as_str());
        let (read_part, write_half) = result.split();

        let result = Self {
            write_part: write_half,
            read_half: H1RemoteConnectionReadPart {
                id: CONN_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
                h1_reader: Mutex::new(Some(H1Reader::new(read_part, HttpTimeouts::default()))),
                disconnected: AtomicBool::new(false),
            }
            .into(),
            ssh_session_handler,
        };

        Ok(result)
    }

    pub fn get_connection_id(&self) -> u64 {
        self.read_half.id
    }

    pub fn is_disconnected(&self) -> bool {
        self.read_half.get_disconnected()
    }

    pub async fn send_with_timeout(
        &mut self,
        payload: &[u8],
        time_out: Duration,
    ) -> Result<(), NetworkError> {
        return self
            .write_part
            .write_all_with_timeout(payload, time_out)
            .await;
    }

    pub async fn flush_it(&mut self) -> Result<(), NetworkError> {
        self.write_part.flush_it().await
    }

    pub async fn write_to_socket(&mut self, payload: &[u8]) -> Result<(), std::io::Error> {
        return self.write_part.write_to_socket(payload).await;
    }

    pub async fn shutdown_socket(&mut self) {
        self.write_part.shutdown_socket().await
    }

    pub fn get_read_part(&self) -> Arc<H1RemoteConnectionReadPart<TNetworkReadPart>> {
        self.read_half.clone()
    }

    pub async fn get_read_and_write_parts(
        self,
    ) -> (TNetworkReadPart, TNetworkWritePart, LoopBuffer) {
        let mut write_access = self.read_half.h1_reader.lock().await;
        let h1_reader = write_access.take().unwrap();
        let (read_part, loop_buffer) = h1_reader.into_read_part();

        (read_part, self.write_part, loop_buffer)
    }
}
