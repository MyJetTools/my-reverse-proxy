use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicI64, AtomicIsize, Ordering},
        Arc,
    },
};

use encryption::aes::AesKey;
use my_ssh::SshAsyncChannel;
use my_tls::tokio_rustls::client::TlsStream;
use rust_extensions::{AppStates, UnsafeValue};
use tokio::{net::TcpStream, sync::Mutex};

use crate::{
    configurations::*,
    http2_client_pool::Http2ClientPool,
    http_client_connectors::*,
    http_client_pool::HttpClientPool,
    settings::{ConnectionsSettingsModel, SettingsModel},
    ssl::CertificatesCache,
    tcp_gateway::{client::TcpGatewayClient, server::TcpGatewayServer, TcpGatewayConnection},
};

use super::{ActiveListenPorts, CertPassKeys, Metrics, Prometheus};

pub const APP_NAME: &'static str = env!("CARGO_PKG_NAME");
pub const APP_VERSION: &'static str = env!("CARGO_PKG_VERSION");

pub struct AppContext {
    pub http_clients_pool: HttpClientPool<TcpStream, HttpConnector>,
    pub http_over_ssh_clients_pool: HttpClientPool<SshAsyncChannel, HttpOverSshConnector>,
    pub https_clients_pool: HttpClientPool<TlsStream<TcpStream>, HttpTlsConnector>,

    pub http2_clients_pool: Http2ClientPool<TcpStream, HttpConnector>,
    pub http2_over_ssh_clients_pool: Http2ClientPool<SshAsyncChannel, HttpOverSshConnector>,
    pub https2_clients_pool: Http2ClientPool<TlsStream<TcpStream>, HttpTlsConnector>,

    pub http_connections: AtomicIsize,
    id: AtomicI64,
    pub connection_settings: ConnectionsSettingsModel,

    pub token_secret_key: AesKey,
    pub current_configuration: AppConfiguration,
    pub states: Arc<AppStates>,

    pub show_error_description: UnsafeValue<bool>,
    pub prometheus: Arc<Prometheus>,
    pub metrics: Metrics,
    pub active_listen_ports: Mutex<ActiveListenPorts>,

    pub ssh_config_list: SshConfigList,

    pub allowed_users_list: AllowedUsersList,

    pub ssl_certificates_cache: CertificatesCache,

    pub ssh_cert_pass_keys: CertPassKeys,

    pub gateway_server: Option<TcpGatewayServer>,
    pub gateway_clients: HashMap<String, TcpGatewayClient>,
}

impl AppContext {
    pub fn new(settings_model: SettingsModel) -> Self {
        let connection_settings = settings_model.get_connections_settings();

        let token_secret_key = if let Some(session_key) = settings_model.get_session_key() {
            AesKey::new(get_token_secret_key_from_settings(session_key.as_bytes()).as_slice())
        } else {
            AesKey::new(generate_random_token_secret_key().as_slice())
        };

        let gateway_server =
            if let Some(gateway_server_settings) = settings_model.get_gateway_server() {
                let encryption = gateway_server_settings.get_encryption_key().unwrap();
                Some(TcpGatewayServer::new(
                    format!("0.0.0.0:{}", gateway_server_settings.port,),
                    encryption,
                    gateway_server_settings.is_debug(),
                ))
            } else {
                None
            };

        let mut gateway_clients = HashMap::new();

        if let Some(clients_settings) = &settings_model.gateway_clients {
            for (id, client_settings) in clients_settings.iter() {
                let encryption = client_settings.get_encryption_key().unwrap();
                let client = TcpGatewayClient::new(
                    id.to_string(),
                    client_settings.remote_host.to_string(),
                    encryption,
                    client_settings.get_supported_compression(),
                    client_settings.is_debug(),
                );

                gateway_clients.insert(id.clone(), client);
            }
        }

        Self {
            http_connections: AtomicIsize::new(0),
            id: AtomicI64::new(0),
            connection_settings,
            // saved_client_certs: SavedClientCert::new(),
            token_secret_key,
            current_configuration: AppConfiguration::new(),
            states: Arc::new(AppStates::create_initialized()),
            prometheus: Arc::new(Prometheus::new()),
            ssl_certificates_cache: CertificatesCache::new(),
            //local_port_allocator: LocalPortAllocator::new(),
            //ssh_to_http_port_forward_pool: SshToHttpPortForwardPool::new(),
            show_error_description: UnsafeValue::new(
                settings_model.get_show_error_description_on_error_page(),
            ),
            metrics: Metrics::new(),
            active_listen_ports: Mutex::new(ActiveListenPorts::new()),
            ssh_config_list: SshConfigList::new(),
            allowed_users_list: AllowedUsersList::new(),
            ssh_cert_pass_keys: CertPassKeys::new(),
            http_clients_pool: HttpClientPool::new(),
            https_clients_pool: HttpClientPool::new(),
            http2_clients_pool: Http2ClientPool::new(),
            https2_clients_pool: Http2ClientPool::new(),
            http_over_ssh_clients_pool: HttpClientPool::new(),
            http2_over_ssh_clients_pool: Http2ClientPool::new(),
            gateway_server: gateway_server,
            gateway_clients: gateway_clients,
        }
    }

    pub fn get_next_id(&self) -> i64 {
        self.id.fetch_add(1, Ordering::SeqCst)
    }

    pub async fn get_gateway_by_id_with_next_connection_id(
        &self,
        gateway_id: &str,
    ) -> Option<(Arc<TcpGatewayConnection>, u32)> {
        if let Some(server_gateway) = self.gateway_server.as_ref() {
            if let Some(result) = server_gateway.get_gateway_connection(gateway_id).await {
                return Some((result, server_gateway.get_next_connection_id()));
            }
        }

        let gateway_client = self.gateway_clients.get(gateway_id)?;

        let result = gateway_client.get_gateway_connection(gateway_id).await?;

        Some((result, gateway_client.get_next_connection_id()))
    }

    pub async fn get_gateway_by_id(&self, gateway_id: &str) -> Option<Arc<TcpGatewayConnection>> {
        if let Some(server_gateway) = self.gateway_server.as_ref() {
            if let Some(result) = server_gateway.get_gateway_connection(gateway_id).await {
                return Some(result);
            }
        }

        let gateway_client = self.gateway_clients.get(gateway_id)?;

        gateway_client.get_gateway_connection(gateway_id).await
    }
}

fn generate_random_token_secret_key() -> Vec<u8> {
    let mut result = Vec::with_capacity(48);

    let mut key = vec![];

    while result.len() < 48 {
        if key.len() == 0 {
            key = uuid::Uuid::new_v4().as_bytes().to_vec();
        }

        result.push(key.pop().unwrap());
    }

    result
}

fn get_token_secret_key_from_settings(session_key: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(48);

    let mut key = vec![];

    while result.len() < 48 {
        if key.len() == 0 {
            key = session_key.to_vec();
        }

        result.push(key.pop().unwrap());
    }

    result
}
